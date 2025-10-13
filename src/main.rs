use anyhow::Result;
use eframe::{egui};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2, Key};
use glob::glob;
use image::GenericImageView;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct BBox {
    class_name: String,
    cx: f32, // center x (ratio 0..1)
    cy: f32, // center y (ratio)
    w: f32,  // width (ratio)
    h: f32,  // height (ratio)
}

struct ImageEntry {
    path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResizeCorner { TL, TR, BL, BR }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode { None, Creating, Moving, Resizing(ResizeCorner) }

struct AppState {
    images: Vec<ImageEntry>,
    cur_idx: usize,
    texture: Option<egui::TextureHandle>,
    texture_size: Vec2, // displayed size in UI
    original_size: (u32, u32),
    dragging: bool,
    drag_start: Pos2,
    drag_end: Pos2,
    boxes: Vec<BBox>,
    classes: Vec<String>,
    cur_class_idx: usize,
    load_dir: PathBuf,
    selected_box: Option<usize>,
    // persistent text field for adding classes (was previously recreated every frame)
    new_class: String,
    drag_mode: DragMode,
    last_pointer_pos: Option<Pos2>,
    // history stack for undo
    history: Vec<Vec<BBox>>,
    history_limit: usize,
    // UI-adjustable settings
    click_tolerance: f32, // pixels; how close a click near the box counts as clicking it
    min_box_pixels: f32,  // min width or height in screen pixels to accept new box
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            images: vec![],
            cur_idx: 0,
            texture: None,
            texture_size: Vec2::ZERO,
            original_size: (0, 0),
            dragging: false,
            drag_start: Pos2::ZERO,
            drag_end: Pos2::ZERO,
            boxes: vec![],
            classes: vec!["object".to_owned()],
            cur_class_idx: 0,
            load_dir: PathBuf::new(),
            selected_box: None,
            new_class: String::new(),
            drag_mode: DragMode::None,
            last_pointer_pos: None,
            history: vec![],
            history_limit: 200,
            click_tolerance: 8.0,
            min_box_pixels: 6.0,
        }
    }
}

impl AppState {
    fn push_history(&mut self) {
        // push current boxes snapshot
        self.history.push(self.boxes.clone());
        if self.history.len() > self.history_limit {
            self.history.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.boxes = prev;
            self.selected_box = None;
            let _ = self.save_annotations_for_current();
        }
    }

    fn classes_file_path(&self) -> PathBuf {
        self.load_dir.join("_darknet.labels")
    }

    fn load_classes_file(&mut self) {
        let path = self.classes_file_path();
        if path.exists() {
            if let Ok(file) = File::open(&path) {
                let reader = BufReader::new(file);
                self.classes.clear();
                for line in reader.lines().flatten() {
                    let s = line.trim().to_string();
                    if !s.is_empty() {
                        self.classes.push(s);
                    }
                }
                if self.classes.is_empty() {
                    self.classes.push("object".to_owned());
                }
            }
        }
    }

    fn save_classes_file(&self) -> Result<()> {
        let path = self.classes_file_path();
        let mut file = File::create(&path)?;
        for c in &self.classes {
            writeln!(file, "{}", c)?;
        }
        Ok(())
    }

    fn load_images_from_dir(dir: &Path) -> Result<Vec<ImageEntry>> {
        let mut imgs = vec![];
        let patterns = ["*.png", "*.jpg", "*.jpeg", "*.bmp", "*.webp", "*.tif"];
        for pat in patterns.iter() {
            let globpat = dir.join(pat).to_string_lossy().to_string();
            for entry in glob(&globpat)? {
                if let Ok(p) = entry {
                    imgs.push(ImageEntry { path: p });
                }
            }
        }
        imgs.sort_by_key(|e| e.path.clone());
        Ok(imgs)
    }

    fn load_current_image_texture(&mut self, ctx: &egui::Context) -> Result<()> {
        self.texture = None;
        self.selected_box = None;
        self.drag_mode = DragMode::None;
        self.last_pointer_pos = None;
        if self.images.is_empty() {
            return Ok(());
        }
        let p = &self.images[self.cur_idx].path;
        let dynimg = image::io::Reader::open(p)?.decode()?;
        let (w, h) = dynimg.dimensions();
        self.original_size = (w, h);
        let rgba = dynimg.to_rgba8();
        let size = [w as usize, h as usize];
        let pixels: Vec<u8> = rgba.into_vec();
        let image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        let tex = ctx.load_texture(p.to_string_lossy(), image, egui::TextureOptions::NEAREST);
        self.texture = Some(tex);
        self.load_annotations_for_current();
        Ok(())
    }

    fn annotation_path_for_image(path: &Path) -> PathBuf {
        let mut out = path.to_path_buf();
        out.set_extension("txt");
        out
    }

    // Attempt to parse annotation files that may contain either class_id or class_name
    fn load_annotations_for_current(&mut self) {
        self.boxes.clear();
        if self.images.is_empty() {
            return;
        }
        let imgp = &self.images[self.cur_idx].path;
        let annp = Self::annotation_path_for_image(imgp);
        if !annp.exists() {
            return;
        }
        if let Ok(file) = File::open(annp) {
            let reader = BufReader::new(file);
            let addition: usize = if self.classes.get(0).is_some_and(|c| c == "object") { 1 } else { 0 };
            for line in reader.lines().flatten() {
                let line = line.trim();
                if line.is_empty() { continue; }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let token = parts[0];
                    // parse numbers as id if possible
                    let class_name = if let Ok(mut id) = token.parse::<usize>() {
                        id += addition;
                        if id < self.classes.len() {
                            self.classes[id].clone()
                        } else {
                            // Unknown id -> create placeholder name and extend classes vector
                            while self.classes.len() <= id { self.classes.push(format!("class_{}", self.classes.len())); }
                            self.classes[id].clone()
                        }
                    } else {
                        token.replace('_', " ")
                    };

                    if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                        parts[1].parse::<f32>(),
                        parts[2].parse::<f32>(),
                        parts[3].parse::<f32>(),
                        parts[4].parse::<f32>(),
                    ) {
                        self.boxes.push(BBox { class_name: class_name.clone(), cx: x, cy: y, w, h });
                        if !self.classes.iter().any(|c| c == &class_name) {
                            self.classes.push(class_name);
                        }
                    }
                }
            }
        }
        // save classes file so newly discovered classes persist
        let _ = self.save_classes_file();
    }

    // save annotations using class ids (index in self.classes)
    fn save_annotations_for_current(&mut self) -> Result<()> {
        if self.images.is_empty() { return Ok(()); }
        let imgp = &self.images[self.cur_idx].path;
        let annp = Self::annotation_path_for_image(imgp);
        let mut file = File::create(&annp)?;
        for b in &self.boxes {
            // find class id or create it
            let mut minus: usize = if self.classes.get(0).is_some_and(|c| c == "object") { 1 } else { 0 };
            let cid = match self.classes.iter().position(|c| c == &b.class_name) {
                Some(i) => i,
                None => {
                    let i = self.classes.len();
                    self.classes.push(b.class_name.clone());
                    // update classes file on disk
                    let _ = self.save_classes_file();
                    i
                }
            };
            if cid < minus {
                // should not happen, but just in case
                minus = 0;
            }
            writeln!(file, "{} {:.6} {:.6} {:.6} {:.6}", cid - minus, b.cx, b.cy, b.w, b.h)?;
        }
        Ok(())
    }

    fn add_box_from_drag(&mut self, img_rect: Rect) {
        let x0 = (self.drag_start.x - img_rect.left()).clamp(0.0, img_rect.width());
        let y0 = (self.drag_start.y - img_rect.top()).clamp(0.0, img_rect.height());
        let x1 = (self.drag_end.x - img_rect.left()).clamp(0.0, img_rect.width());
        let y1 = (self.drag_end.y - img_rect.top()).clamp(0.0, img_rect.height());
        let nx0 = x0.min(x1) / img_rect.width();
        let ny0 = y0.min(y1) / img_rect.height();
        let nx1 = x0.max(x1) / img_rect.width();
        let ny1 = y0.max(y1) / img_rect.height();
        let w = (nx1 - nx0).max(0.0);
        let h = (ny1 - ny0).max(0.0);
        let cx = (nx0 + nx1) / 2.0;
        let cy = (ny0 + ny1) / 2.0;
        // check pixel size threshold
        let pixel_w = w * img_rect.width();
        let pixel_h = h * img_rect.height();
        if w > 0.0 && h > 0.0 && pixel_w >= self.min_box_pixels && pixel_h >= self.min_box_pixels {
            let class_name = if self.classes.is_empty() {
                "object".to_owned()
            } else {
                self.classes.get(self.cur_class_idx).cloned().unwrap_or_else(|| self.classes[0].clone())
            };
            // record history before creating
            self.push_history();
            self.boxes.push(BBox { class_name, cx, cy, w, h });
        }
    }

    // fn screen_to_ratio(&self, pos: Pos2, img_rect: Rect) -> (f32, f32) {
    //     let x = ((pos.x - img_rect.left()) / img_rect.width()).clamp(0.0, 1.0);
    //     let y = ((pos.y - img_rect.top()) / img_rect.height()).clamp(0.0, 1.0);
    //     (x, y)
    // }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // handle Ctrl+Z undo
        let ctrl_z_pressed = ctx.input(|input| input.modifiers.ctrl && input.key_pressed(Key::Z));
        if ctrl_z_pressed {
            self.undo();
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Prev").clicked() {
                    if !self.images.is_empty() {
                        let _ = self.save_annotations_for_current();
                        if self.cur_idx == 0 { self.cur_idx = self.images.len() - 1; }
                        else { self.cur_idx -= 1; }
                        let _ = self.load_current_image_texture(ctx);
                    }
                }
                if ui.button("Next").clicked() {
                    if !self.images.is_empty() {
                        let _ = self.save_annotations_for_current();
                        self.cur_idx = (self.cur_idx + 1) % self.images.len();
                        let _ = self.load_current_image_texture(ctx);
                    }
                }

                if ui.button("Save").clicked() {
                    let _ = self.save_annotations_for_current();
                }

                ui.label(format!("Image {}/{}", self.cur_idx + 1, self.images.len().max(1)));

                ui.separator();

                if ui.button("Reload folder").clicked() {
                    if let Ok(list) = Self::load_images_from_dir(&self.load_dir) {
                        self.images = list;
                        self.cur_idx = 0;
                        // reload classes and first image
                        self.load_classes_file();
                        let _ = self.load_current_image_texture(ctx);
                    }
                }

                if ui.button("Quit").clicked() { std::process::exit(0); }
            });
        });

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Classes");
                if !self.classes.is_empty() {
                    let mut idx = self.cur_class_idx.min(self.classes.len()-1);
                    egui::ComboBox::from_id_source("class_combo")
                        .selected_text(self.classes[idx].clone())
                        .show_ui(ui, |ui| {
                            for (i, c) in self.classes.iter().enumerate() {
                                if ui.selectable_label(i==idx, c).clicked() { idx = i; }
                            }
                        });
                    self.cur_class_idx = idx;
                }

                ui.separator();
                ui.label("Add new class:");
                ui.horizontal(|ui| {
                    // use persistent `self.new_class` so the text field isn't reset each frame
                    ui.text_edit_singleline(&mut self.new_class);
                    if ui.button("Add").clicked() {
                        if !self.new_class.trim().is_empty() {
                            let name = self.new_class.trim().to_owned();
                            if !self.classes.iter().any(|c| c == &name) {
                                self.classes.push(name.clone());
                                // persist classes
                                let _ = self.save_classes_file();
                            }
                            self.cur_class_idx = self.classes.iter().position(|c| c == &name).unwrap_or(0);
                            self.new_class.clear();
                        }
                    }
                });

                ui.separator();
                ui.label("Settings:");
                ui.add(egui::Slider::new(&mut self.click_tolerance, 1.0..=30.0).text("click tolerance (px)"));
                ui.add(egui::Slider::new(&mut self.min_box_pixels, 1.0..=40.0).text("min box pixels"));

                ui.separator();
                ui.heading("Images in folder:");
                // Collect clicked index outside the loop to avoid borrow issues
                let mut clicked_idx: Option<usize> = None;
                for (i, e) in self.images.iter().enumerate() {
                    let fname = e.path.file_name().unwrap().to_string_lossy();
                    if ui.selectable_label(i == self.cur_idx, fname.as_ref()).clicked() {
                        clicked_idx = Some(i);
                    }
                }
                if let Some(i) = clicked_idx {
                    let _ = self.save_annotations_for_current();
                    self.cur_idx = i;
                    let _ = self.load_current_image_texture(ctx);
                }
            })
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(if self.images.is_empty() { "No images loaded. Launch with: cargo run -- /path/to/images" } else { "Draw bounding boxes by clicking and dragging over the image. Click a box to select it, drag to move, drag corners to resize. Ctrl+Z to undo." });

            if self.images.is_empty() { return; }

            if let Some(tex) = &self.texture {
                let available = ui.available_size();
                let (ow, oh) = self.original_size;
                let ow = ow as f32; let oh = oh as f32;
                let mut dw = available.x; let mut dh = available.y - 10.0;
                let aspect = ow / oh;
                if dw / dh > aspect { dw = dh * aspect; } else { dh = dw / aspect; }
                let image_size = Vec2::new(dw, dh);
                self.texture_size = image_size;

                let image = egui::Image::new(tex).fit_to_exact_size(image_size);
                let resp = ui.add(image.sense(Sense::click_and_drag()));
                let img_rect = resp.rect;

                let pointer = ui.input(|i| i.pointer.clone());

                // handle press
                if pointer.primary_clicked() {
                    if let Some(pos) = pointer.interact_pos() {
                        if img_rect.contains(pos) {
                            // Check if click is inside a box (with tolerance)
                            let mut found = None;
                            for (i, b) in self.boxes.iter().enumerate().rev() {
                                let left = img_rect.left() + (b.cx - b.w/2.0) * img_rect.width();
                                let top = img_rect.top() + (b.cy - b.h/2.0) * img_rect.height();
                                let right = left + b.w * img_rect.width();
                                let bottom = top + b.h * img_rect.height();
                                let tol = self.click_tolerance;
                                if pos.x >= left - tol && pos.x <= right + tol && pos.y >= top - tol && pos.y <= bottom + tol {
                                    found = Some(i);
                                    break;
                                }
                            }
                            self.selected_box = found;

                            // If user clicked on a box, decide move or resize; otherwise start creating
                            if let Some(i) = found {
                                // record history once when action starts
                                self.push_history();

                                // determine corner proximity
                                let b = &self.boxes[i];
                                let left = img_rect.left() + (b.cx - b.w/2.0) * img_rect.width();
                                let top = img_rect.top() + (b.cy - b.h/2.0) * img_rect.height();
                                let right = left + b.w * img_rect.width();
                                let bottom = top + b.h * img_rect.height();
                                let handle = self.click_tolerance.max(6.0); // use tolerance as handle size but at least 6px
                                let near_left = (pos.x - left).abs() <= handle;
                                let near_right = (pos.x - right).abs() <= handle;
                                let near_top = (pos.y - top).abs() <= handle;
                                let near_bottom = (pos.y - bottom).abs() <= handle;
                                self.last_pointer_pos = Some(pos);
                                if near_left && near_top { self.drag_mode = DragMode::Resizing(ResizeCorner::TL); }
                                else if near_right && near_top { self.drag_mode = DragMode::Resizing(ResizeCorner::TR); }
                                else if near_left && near_bottom { self.drag_mode = DragMode::Resizing(ResizeCorner::BL); }
                                else if near_right && near_bottom { self.drag_mode = DragMode::Resizing(ResizeCorner::BR); }
                                else { self.drag_mode = DragMode::Moving; }
                            } else {
                                self.drag_mode = DragMode::Creating;
                                if let Some(p) = pointer.interact_pos() {
                                    self.dragging = true;
                                    self.drag_start = p;
                                    self.drag_end = p;
                                    // record history for creation start
                                    self.push_history();
                                }
                            }
                        }
                    }
                }

                // handle drag updates
                if self.drag_mode != DragMode::None {
                    if self.drag_mode == DragMode::Creating {
                        if let Some(pos) = pointer.interact_pos() { self.drag_end = pos; }
                    } else if self.drag_mode == DragMode::Moving {
                        if pointer.primary_down() {
                            if let Some(pos) = pointer.interact_pos() {
                                if let Some(last) = self.last_pointer_pos {
                                    let dx = (pos.x - last.x) / img_rect.width();
                                    let dy = (pos.y - last.y) / img_rect.height();
                                    if let Some(idx) = self.selected_box {
                                        if let Some(b) = self.boxes.get_mut(idx) {
                                            b.cx = (b.cx + dx).clamp(0.0, 1.0);
                                            b.cy = (b.cy + dy).clamp(0.0, 1.0);
                                        }
                                    }
                                    self.last_pointer_pos = Some(pos);
                                } else {
                                    self.last_pointer_pos = pointer.interact_pos();
                                }
                            }
                        }
                    } else if let DragMode::Resizing(corner) = self.drag_mode {
                        if pointer.primary_down() {
                            if let Some(pos) = pointer.interact_pos() {
                                // compute opposite corner fixed, and new coords
                                if let Some(idx) = self.selected_box {
                                    if let Some(b) = self.boxes.get_mut(idx) {
                                        // get current box corners in image ratios
                                        let left = b.cx - b.w/2.0;
                                        let right = b.cx + b.w/2.0;
                                        let top = b.cy - b.h/2.0;
                                        let bottom = b.cy + b.h/2.0;
                                        // extract img_rect values before mutable borrow
                                        let img_left = img_rect.left();
                                        let img_top = img_rect.top();
                                        let img_width = img_rect.width();
                                        let img_height = img_rect.height();
                                        // pointer to ratios
                                        let (rx, ry) = {
                                            let x = ((pos.x - img_left) / img_width).clamp(0.0, 1.0);
                                            let y = ((pos.y - img_top) / img_height).clamp(0.0, 1.0);
                                            (x, y)
                                        };
                                        let (new_left, new_top, new_right, new_bottom) = match corner {
                                            ResizeCorner::TL => (rx, ry, right, bottom),
                                            ResizeCorner::TR => (left, ry, rx, bottom),
                                            ResizeCorner::BL => (rx, top, right, ry),
                                            ResizeCorner::BR => (left, top, rx, ry),
                                        };
                                        // normalize
                                        let nl = new_left.min(new_right);
                                        let nr = new_left.max(new_right);
                                        let nt = new_top.min(new_bottom);
                                        let nb = new_top.max(new_bottom);
                                        let nw = (nr - nl).max(0.001);
                                        let nh = (nb - nt).max(0.001);
                                        b.cx = (nl + nr) / 2.0;
                                        b.cy = (nt + nb) / 2.0;
                                        b.w = nw.clamp(0.0001, 1.0);
                                        b.h = nh.clamp(0.0001, 1.0);
                                    }
                                }
                            }
                        }
                    }
                }

                // handle release
                if pointer.primary_released() && self.drag_mode != DragMode::None {
                    if self.drag_mode == DragMode::Creating {
                        self.dragging = false;
                        self.add_box_from_drag(img_rect);
                        let _ = self.save_annotations_for_current();
                    } else {
                        // moving or resizing ended, save
                        let _ = self.save_annotations_for_current();
                    }
                    self.drag_mode = DragMode::None;
                    self.last_pointer_pos = None;
                }

                // draw boxes
                let painter = ui.painter();
                for (i, b) in self.boxes.iter().enumerate() {
                    let left = img_rect.left() + (b.cx - b.w / 2.0) * img_rect.width();
                    let top = img_rect.top() + (b.cy - b.h / 2.0) * img_rect.height();
                    let right = left + b.w * img_rect.width();
                    let bottom = top + b.h * img_rect.height();
                    let r = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                    if Some(i) == self.selected_box {
                        painter.rect_stroke(r, 0.0, Stroke::new(3.0, Color32::from_rgb(255, 50, 50)));
                        // draw corner handles
                        let hs = 6.0;
                        painter.rect_filled(Rect::from_min_max(Pos2::new(left-hs, top-hs), Pos2::new(left+hs, top+hs)), 0.0, Color32::WHITE);
                        painter.rect_filled(Rect::from_min_max(Pos2::new(right-hs, top-hs), Pos2::new(right+hs, top+hs)), 0.0, Color32::WHITE);
                        painter.rect_filled(Rect::from_min_max(Pos2::new(left-hs, bottom-hs), Pos2::new(left+hs, bottom+hs)), 0.0, Color32::WHITE);
                        painter.rect_filled(Rect::from_min_max(Pos2::new(right-hs, bottom-hs), Pos2::new(right+hs, bottom+hs)), 0.0, Color32::WHITE);
                    } else {
                        painter.rect_stroke(r, 0.0, Stroke::new(2.0, Color32::from_rgb(200, 100, 50)));
                    }
                    // show class name and id
                    let class_id = self.classes.iter().position(|c| c==&b.class_name).unwrap_or(0);
                    painter.text(Pos2::new(left + 2.0, top + 2.0), egui::Align2::LEFT_TOP, format!("{}:{}", class_id, &b.class_name), egui::TextStyle::Body.resolve(&ui.style()), Color32::WHITE);
                }

                if self.dragging && self.drag_mode == DragMode::Creating {
                    let x0 = self.drag_start.x.clamp(img_rect.left(), img_rect.right());
                    let y0 = self.drag_start.y.clamp(img_rect.top(), img_rect.bottom());
                    let x1 = self.drag_end.x.clamp(img_rect.left(), img_rect.right());
                    let y1 = self.drag_end.y.clamp(img_rect.top(), img_rect.bottom());
                    let r = Rect::from_min_max(Pos2::new(x0.min(x1), y0.min(y1)), Pos2::new(x0.max(x1), y0.max(y1)));
                    painter.rect_stroke(r, 0.0, Stroke::new(2.0, Color32::from_rgb(100, 200, 200)));
                }

                let tools_pos = Pos2::new(img_rect.right() - 10.0, img_rect.top() + 10.0);
                egui::Area::new("tools_area").fixed_pos(tools_pos).show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if ui.button("Delete Selected Box").clicked() {
                            if let Some(idx) = self.selected_box {
                                if idx < self.boxes.len() {
                                    self.push_history();
                                    self.boxes.remove(idx);
                                    self.selected_box = None;
                                    let _ = self.save_annotations_for_current();
                                }
                            }
                        }

                        if ui.button("Duplicate Selected Box").clicked() {
                            if let Some(idx) = self.selected_box {
                                self.push_history();
                                if let Some(b) = self.boxes.get(idx) {
                                    self.boxes.push(b.clone());
                                    let _ = self.save_annotations_for_current();
                                }
                            }
                        }

                        ui.separator();
                        ui.label("Selected box controls:");
                        if let Some(idx) = self.selected_box {
                            // Move push_history before any borrow of self.boxes
                            self.push_history();
                            let mut sel = self.classes.iter().position(|c| {
                                if let Some(b) = self.boxes.get(idx) {
                                    c == &b.class_name
                                } else {
                                    false
                                }
                            }).unwrap_or(0);
                            if let Some(b) = self.boxes.get_mut(idx) {
                                // choose class from existing classes (no need to re-type previously used names)
                                egui::ComboBox::from_id_source("selected_class_combo")
                                    .selected_text(self.classes[sel].clone())
                                    .show_ui(ui, |ui| {
                                        for (i, c) in self.classes.iter().enumerate() {
                                            if ui.selectable_label(i==sel, c).clicked() { sel = i; }
                                        }
                                    });
                                let mut need_save = false;
                                if b.class_name != self.classes[sel] {
                                    let new_class_name = self.classes[sel].clone();
                                    b.class_name = new_class_name;
                                    need_save = true;
                                }
                                // allow quick reassign to current default class as well
                                if ui.button("Assign current left-class to selected").clicked() {
                                    let new_class_name = self.classes[self.cur_class_idx].clone();
                                    b.class_name = new_class_name;
                                    need_save = true;
                                }
                                // Save after mutable borrow ends
                                if need_save {
                                    let _ = self.save_annotations_for_current();
                                }
                            }
                        } else {
                            ui.label("No box selected.");
                        }
                    });
                });

            }
        });
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut app = AppState::default();
    if args.len() >= 2 {
        let dir = PathBuf::from(&args[1]);
        if dir.is_dir() {
            app.load_dir = dir.clone();
            // load classes first (persisted file)
            app.load_classes_file();
            match AppState::load_images_from_dir(&dir) {
                Ok(list) => { app.images = list; }
                Err(e) => { eprintln!("Failed to read images: {}", e); }
            }
        } else { eprintln!("Provided path is not a directory: {}", dir.display()); }
    } else { eprintln!("Usage: cargo run -- /path/to/images"); }

    let native_options = eframe::NativeOptions::default();
    // set visuals during creation
    let _ = eframe::run_native("Rust Image Annotator", native_options, Box::new(move |cc| {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        // ensure we load the first image texture now that we have a ctx
        let _ = app.load_current_image_texture(&cc.egui_ctx);
        Box::new(app)
    }));

    Ok(())
}
