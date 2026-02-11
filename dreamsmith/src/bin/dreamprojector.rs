use anyhow::Result;
use binrw::BinRead;
use dreamsmith::storybook::*;
use eframe::egui;
use rodio::{OutputStream, OutputStreamBuilder, Sink, buffer::SamplesBuffer};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

#[derive(PartialEq)]
enum AsyncCommand {
    PlayFromStart,
}

struct DreamProjectorApp {
    book: StoryBook,
    current_page: usize,

    // Pre-decoded PCM for each page
    pcm_cache: Vec<Vec<i16>>,

    // Audio playback (None if no audio device available)
    audio_output: Option<OutputStream>,
    sink: Option<Sink>,
    playing: bool,
    playback_start: Option<Instant>,
    pause_offset_ms: f64,

    // channel for multi-threaded callbacks
    rx: Receiver<AsyncCommand>,
    tx: Sender<AsyncCommand>,

    // Per-page derived data
    page_durations_ms: Vec<f64>,
    page_color_sequences: Vec<Vec<(u8, u8, u8)>>,
}

impl DreamProjectorApp {
    fn new(book: StoryBook) -> Self {
        let pcm_cache: Vec<Vec<i16>> = book.pages.iter().map(|p| p.audio.decode_to_pcm()).collect();
        let page_durations_ms: Vec<f64> =
            book.pages.iter().map(|p| p.audio.duration_ms()).collect();
        let page_color_sequences: Vec<Vec<(u8, u8, u8)>> = book
            .pages
            .iter()
            .map(|p| match &p.lights {
                None => vec![],
                Some(lights) => lights.get_color_sequence(),
            })
            .collect();
        let audio_output = match OutputStreamBuilder::open_default_stream() {
            Ok(stream) => Some(stream),
            Err(e) => {
                eprintln!("Warning: no audio output available ({e}), playback disabled");
                None
            }
        };
        let (tx, rx) = channel::<AsyncCommand>();
        Self {
            book,
            current_page: 0,
            pcm_cache,
            audio_output,
            sink: None,
            playing: false,
            playback_start: None,
            pause_offset_ms: 0.0,
            rx,
            tx,
            page_durations_ms,
            page_color_sequences,
        }
    }

    fn current_position_ms(&self) -> f64 {
        if self.playing {
            if let Some(start) = self.playback_start {
                self.pause_offset_ms + start.elapsed().as_secs_f64() * 1000.0
            } else {
                self.pause_offset_ms
            }
        } else {
            self.pause_offset_ms
        }
    }

    fn play_from(&mut self, offset_ms: f64) {
        self.stop_audio();

        let Some(ref audio_output) = self.audio_output else {
            // No audio device — advance timeline without sound
            self.playing = true;
            self.pause_offset_ms = offset_ms;
            self.playback_start = Some(Instant::now());
            return;
        };

        let pcm = &self.pcm_cache[self.current_page];
        // Skip samples corresponding to offset_ms
        // 16000 samples/sec = 16 samples/ms
        let skip_samples = (offset_ms * 16.0) as usize;
        if skip_samples >= pcm.len() {
            return;
        }

        let remaining = pcm[skip_samples..]
            .to_vec()
            .iter()
            .map(|s| *s as f32 / 32767.0)
            .collect::<Vec<_>>();

        let source = SamplesBuffer::new(1, 16000, remaining);
        let sink = Sink::connect_new(audio_output.mixer());
        sink.append(source);

        self.sink = Some(sink);
        self.playing = true;
        self.pause_offset_ms = offset_ms;
        self.playback_start = Some(Instant::now());
    }

    fn toggle_play_pause(&mut self) {
        if self.playing {
            // Pause
            let pos = self.current_position_ms();
            self.stop_audio();
            self.pause_offset_ms = pos;
        } else {
            // Play / resume
            self.play_from(self.pause_offset_ms);
        }
    }

    fn stop_audio(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.playing = false;
        self.playback_start = None;
    }

    fn select_page(&mut self, index: usize) {
        if index >= self.book.pages.len() {
            return;
        }
        self.stop_audio();
        self.current_page = index;
        self.pause_offset_ms = 0.0;
    }

    fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.select_page(self.current_page - 1);
        }
    }

    fn next_page(&mut self) {
        if self.current_page + 1 < self.book.pages.len() {
            self.select_page(self.current_page + 1);
        }
    }

    fn current_led_color(&self) -> (u8, u8, u8) {
        let page = &self.book.pages[self.current_page];
        let Some(_) = page.lights else {
            return (0, 0, 0);
        };

        let index = (self.current_position_ms() / 20.0).floor() as usize; // 20ms per light entry
        let seq = &self.page_color_sequences[self.current_page];
        if index < seq.len() {
            seq[index]
        } else {
            seq.last().cloned().unwrap_or((0, 0, 0))
        }
    }
    fn play_delayed(&self, delay: u64, ctx: egui::Context) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            tx.send(AsyncCommand::PlayFromStart).unwrap();
            // send an event to egui to trigger a repaint and start playback
            ctx.request_repaint();
        });
    }
}

impl eframe::App for DreamProjectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let duration = self.page_durations_ms[self.current_page];
        let mut position = self.current_position_ms().min(duration);

        // Auto-stop when playback reaches the end
        if self.playing && position >= duration {
            self.stop_audio();
            self.pause_offset_ms = 0.0;
            position = 0.0;
            // Auto-advance to next page
            if self.current_page + 1 < self.book.pages.len() {
                self.current_page += 1;
                // auto-play next page
                self.play_delayed(2000, ctx.clone());
            }
        }

        // handle async commands (e.g. from delayed play)
        if let Ok(cmd) = self.rx.try_recv()
            && cmd == AsyncCommand::PlayFromStart
        {
            self.play_from(0.0);
        }
        // uncomment to see bounding boxes when hovering over elements (useful for debugging layout issues)
        /*      ctx.set_debug_on_hover(true);
                ctx.style_mut(|style| {
                    style.debug.show_expand_width = true;
                    style.debug.show_expand_height = true;
                    style.debug.show_resize = true;
                });
        */
        egui::CentralPanel::default().show(ctx, |ui| {
            // --- Page list + LED circle side by side ---
            ui.horizontal(|ui| {
                // Left: page list
                ui.vertical(|ui| {
                    ui.heading("Pages");
                    ui.set_min_height(200.0);
                    egui::ScrollArea::vertical()
                        .max_height(f32::INFINITY)
                        .show(ui, |ui| {
                            for i in 0..self.book.pages.len() {
                                let dur_s = self.page_durations_ms[i] / 1000.0;
                                let label = format!("Page {} ({:.1}s)", i + 1, dur_s);
                                let selected = i == self.current_page;
                                let label = ui.selectable_label(selected, &label);
                                if label.double_clicked() {
                                    self.select_page(i);
                                    self.play_from(0.0);
                                } else if label.clicked() && !selected {
                                    self.select_page(i);
                                }
                            }
                        });
                });

                ui.separator();

                // Right: LED circle
                ui.vertical_centered(|ui| {
                    ui.add_space(25.0);
                    let (r, g, b) = self.current_led_color();
                    let color = egui::Color32::from_rgb(r, g, b);

                    let radius = 60.0;
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(radius * 2.0, radius * 2.0),
                        egui::Sense::hover(),
                    );
                    let center = rect.center();
                    let painter = ui.painter();

                    painter.circle_stroke(
                        center,
                        radius,
                        egui::Stroke::new(2.0, egui::Color32::GRAY),
                    );
                    painter.circle_filled(center, radius - 2.0, color);

                    ui.label(format!("LED: R={:>3} G={:>3} B={:>3}", r, g, b));
                });
            });

            ui.separator();

            // --- Transport controls --
            ui.horizontal(|ui| {
                if ui.button("\u{23EE} Prev").clicked() {
                    self.prev_page();
                }

                let play_label = if self.playing {
                    "\u{23F8} Pause"
                } else {
                    "\u{25B6} Play"
                };
                if ui.button(play_label).clicked() {
                    self.toggle_play_pause();
                }

                if ui.button("Next \u{23ED}").clicked() {
                    self.next_page();
                }
            });

            // --- Timeline ---
            //ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 0.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let pos_s = position / 1000.0;
                    let dur_s = duration / 1000.0;
                    let label = ui.label(format!(
                        "{:.0}:{:04.1}",
                        (pos_s / 60.0).floor(),
                        pos_s % 60.0
                    ));

                    ui.style_mut().spacing.slider_width =
                        ui.available_width() - 1.5 * label.rect.width();

                    let slider_response = ui.add(
                        egui::Slider::new(&mut position, 0.0..=duration)
                            .show_value(false)
                            .trailing_fill(true),
                    );

                    if slider_response.changed() {
                        // User dragged the slider — seek
                        if self.playing {
                            self.play_from(position);
                        } else {
                            self.pause_offset_ms = position;
                        }
                    }

                    ui.label(format!(
                        "{:.0}:{:04.1}",
                        (dur_s / 60.0).floor(),
                        dur_s % 60.0
                    ));
                },
            );
        });

        // Request continuous repaint while playing
        if self.playing {
            ctx.request_repaint();
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("Usage: dreamprojector <storybook.bin>");

    let image = std::fs::read(path)?;
    let mut cursor = std::io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;

    println!("{book}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([400.0, 400.0])
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };

    let path = std::path::Path::new(path);
    let title = format!(
        "DreamProjector - {}",
        path.file_name().unwrap().to_string_lossy()
    );
    eframe::run_native(
        &title,
        options,
        Box::new(|_cc| Ok(Box::new(DreamProjectorApp::new(book)))),
    )
    .expect("failed to run eframe");

    Ok(())
}
