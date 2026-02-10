use anyhow::Result;
use binrw::BinRead;
use eframe::egui;
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};
use std::time::Instant;

use dreamsmith::storybook::*;

struct AudioOutput {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
}

struct DreamProjectorApp {
    book: StoryBook,
    current_chapter: usize,

    // Pre-decoded PCM for each page
    pcm_cache: Vec<Vec<i16>>,

    // Audio playback (None if no audio device available)
    audio_output: Option<AudioOutput>,
    sink: Option<Sink>,
    playing: bool,
    playback_start: Option<Instant>,
    pause_offset_ms: f64,

    // Per-chapter derived data
    chapter_durations_ms: Vec<f64>,
}

impl DreamProjectorApp {
    fn new(book: StoryBook) -> Self {
        let pcm_cache: Vec<Vec<i16>> = book.pages.iter().map(|p| p.audio.decode_to_pcm()).collect();
        let chapter_durations_ms: Vec<f64> = book.pages.iter().map(|p| p.audio.duration_ms()).collect();

        let audio_output = match OutputStream::try_default() {
            Ok((_stream, stream_handle)) => Some(AudioOutput { _stream, stream_handle }),
            Err(e) => {
                eprintln!("Warning: no audio output available ({e}), playback disabled");
                None
            }
        };

        Self {
            book,
            current_chapter: 0,
            pcm_cache,
            audio_output,
            sink: None,
            playing: false,
            playback_start: None,
            pause_offset_ms: 0.0,
            chapter_durations_ms,
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

        let pcm = &self.pcm_cache[self.current_chapter];
        // Skip samples corresponding to offset_ms
        // 16000 samples/sec = 16 samples/ms
        let skip_samples = (offset_ms * 16.0) as usize;
        if skip_samples >= pcm.len() {
            return;
        }
        let remaining = &pcm[skip_samples..];

        let source = SamplesBuffer::new(1, 16000, remaining.to_vec());
        if let Ok(sink) = Sink::try_new(&audio_output.stream_handle) {
            sink.append(source);
            self.sink = Some(sink);
        }

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

    fn select_chapter(&mut self, index: usize) {
        if index >= self.book.pages.len() {
            return;
        }
        self.stop_audio();
        self.current_chapter = index;
        self.pause_offset_ms = 0.0;
    }

    fn prev_chapter(&mut self) {
        if self.current_chapter > 0 {
            self.select_chapter(self.current_chapter - 1);
        }
    }

    fn next_chapter(&mut self) {
        if self.current_chapter + 1 < self.book.pages.len() {
            self.select_chapter(self.current_chapter + 1);
        }
    }

    fn current_led_color(&self) -> (u8, u8, u8) {
        let page = &self.book.pages[self.current_chapter];
        let Some(ref lights) = page.lights else {
            return (0, 0, 0);
        };

        let audio_dur = self.chapter_durations_ms[self.current_chapter];
        let light_dur = lights.total_duration_ms() as f64;
        let light_start = (audio_dur - light_dur).max(0.0);
        let pos = self.current_position_ms();
        let light_offset = pos - light_start;

        if light_offset < 0.0 {
            (0, 0, 0)
        } else {
            lights.color_at(light_offset as u32)
        }
    }
}

impl eframe::App for DreamProjectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let duration = self.chapter_durations_ms[self.current_chapter];
        let mut position = self.current_position_ms().min(duration);

        // Auto-stop when playback reaches the end
        if self.playing && position >= duration {
            self.stop_audio();
            self.pause_offset_ms = 0.0;
            position = 0.0;
            // Auto-advance to next chapter
            if self.current_chapter + 1 < self.book.pages.len() {
                self.current_chapter += 1;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // --- Chapter list ---
            ui.heading("Chapters");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for i in 0..self.book.pages.len() {
                        let dur_s = self.chapter_durations_ms[i] / 1000.0;
                        let label = format!("Chapter {} ({:.1}s)", i + 1, dur_s);
                        let selected = i == self.current_chapter;
                        if ui.selectable_label(selected, &label).clicked() && !selected {
                            self.select_chapter(i);
                        }
                    }
                });

            ui.separator();

            // --- Transport controls ---
            ui.horizontal(|ui| {
                if ui.button("\u{23EE} Prev").clicked() {
                    self.prev_chapter();
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
                    self.next_chapter();
                }
            });

            // --- Timeline ---
            ui.horizontal(|ui| {
                let pos_s = position / 1000.0;
                let dur_s = duration / 1000.0;
                ui.label(format!(
                    "{:.0}:{:04.1}",
                    (pos_s / 60.0).floor(),
                    pos_s % 60.0
                ));

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
            });

            ui.separator();

            // --- LED circle ---
            let (r, g, b) = self.current_led_color();
            let color = egui::Color32::from_rgb(r, g, b);

            let available = ui.available_size();
            let radius = available.x.min(available.y).min(120.0) / 2.0;
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(radius * 2.0, radius * 2.0), egui::Sense::hover());
            let center = rect.center();
            let painter = ui.painter();

            // Draw a border circle
            painter.circle_stroke(center, radius, egui::Stroke::new(2.0, egui::Color32::GRAY));
            // Draw the filled circle
            painter.circle_filled(center, radius - 2.0, color);

            // Show RGB values
            ui.label(format!("LED: R={} G={} B={}", r, g, b));
        });

        // Request continuous repaint while playing
        if self.playing {
            ctx.request_repaint();
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("Usage: dreamprojector <storybook.bin>");

    let image = std::fs::read(path)?;
    let mut cursor = std::io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;

    println!("{book}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "DreamProjector",
        options,
        Box::new(|_cc| Ok(Box::new(DreamProjectorApp::new(book)))),
    )
    .expect("failed to run eframe");

    Ok(())
}
