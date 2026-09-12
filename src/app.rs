use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use rand::Rng;

use crate::geometry::Point;
use crate::maze::{Maze, MazeConfig, generate};

const PATH_WIDTH: f32 = 9.0;
const MARKER_RADIUS: f32 = 5.0;

pub struct MazeApp {
    config: MazeConfig,
    maze: Option<Maze>,
    result_receiver: Option<Receiver<Maze>>,
    generating: bool,
}

impl MazeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = Self {
            config: MazeConfig::default(),
            maze: None,
            result_receiver: None,
            generating: false,
        };
        app.start_generation();
        app
    }

    fn start_generation(&mut self) {
        if self.generating {
            return;
        }
        let config = self.config.clone();
        let (sender, receiver) = mpsc::channel();
        self.result_receiver = Some(receiver);
        self.generating = true;
        std::thread::Builder::new()
            .name("maze-generator".into())
            .spawn(move || {
                let maze = generate(&config);
                let _ = sender.send(maze);
            })
            .expect("failed to start maze generator");
    }

    fn receive_result(&mut self) {
        let result = self
            .result_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(maze) = result {
            self.maze = Some(maze);
            self.result_receiver = None;
            self.generating = false;
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mazui");
        ui.label("Deterministic polygon mazes");
        ui.add_space(14.0);

        ui.label("Random seed");
        ui.add(egui::DragValue::new(&mut self.config.seed).speed(1.0));
        if ui.button("Randomized").clicked() {
            self.config.seed = rand::rng().random();
        }

        ui.separator();
        ui.label("Boundary");
        ui.add(egui::Slider::new(&mut self.config.polygon_sides, 3..=16).text("sides"));
        ui.add(egui::Slider::new(&mut self.config.radius, 150.0..=600.0).text("radius"));

        ui.separator();
        ui.label("Generation");
        ui.add(egui::Slider::new(&mut self.config.unit, 5.0..=30.0).text("unit (px)"));
        ui.add(egui::Slider::new(&mut self.config.max_length_units, 1..=20).text("max line units"));
        ui.add(
            egui::Slider::new(&mut self.config.failure_limit, 100..=5_000)
                .logarithmic(true)
                .text("failure limit"),
        );

        ui.add_space(12.0);
        if ui
            .add_enabled(!self.generating, egui::Button::new("Generate maze"))
            .clicked()
        {
            self.start_generation();
        }

        ui.add_space(12.0);
        if self.generating {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Generating in background…");
            });
        } else if let Some(maze) = &self.maze {
            ui.colored_label(Color32::from_rgb(111, 207, 151), "Ready");
            ui.label(format!("{} path units", maze.edges.len()));
            ui.label(format!("{}-unit longest route", maze.solution_units));
            ui.label(format!("{} attempts", maze.attempts));
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("Start and exit are the two most distant points along the maze network.");
        });
    }

    fn draw_maze(&self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
        let Some(maze) = &self.maze else {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "Building maze…",
                egui::FontId::proportional(18.0),
                Color32::GRAY,
            );
            return;
        };

        let margin = 28.0;
        let canvas = response.rect.shrink(margin);
        let maze_radius = maze
            .boundary
            .iter()
            .map(|point| point.length_squared().sqrt())
            .fold(1.0_f32, f32::max);
        let diameter = maze_radius * 2.0;
        let scale = (canvas.width() / diameter).min(canvas.height() / diameter);
        let to_screen = |point: Point| -> Pos2 {
            Pos2::new(
                canvas.center().x + point.x * scale,
                canvas.center().y - point.y * scale,
            )
        };

        let path_color = Color32::from_rgb(226, 232, 240);
        let boundary_color = Color32::from_rgb(71, 85, 105);
        for i in 0..maze.boundary.len() {
            let a = to_screen(maze.boundary[i]);
            let b = to_screen(maze.boundary[(i + 1) % maze.boundary.len()]);
            painter.line_segment([a, b], Stroke::new(2.0, boundary_color));
        }
        for (a, b) in maze.segments() {
            painter.line_segment(
                [to_screen(a), to_screen(b)],
                Stroke::new(PATH_WIDTH, path_color),
            );
        }

        painter.circle_filled(
            to_screen(maze.nodes[maze.start]),
            MARKER_RADIUS,
            Color32::from_rgb(52, 211, 153),
        );
        painter.circle_filled(
            to_screen(maze.nodes[maze.end]),
            MARKER_RADIUS,
            Color32::from_rgb(251, 113, 133),
        );

        draw_legend(&painter, response.rect);
    }
}

impl eframe::App for MazeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive_result();
        if self.generating {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }

        egui::Panel::right("controls")
            .resizable(false)
            .exact_size(260.0)
            .show(ui, |ui| {
                ui.add_space(14.0);
                self.controls(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| self.draw_maze(ui));
    }
}

fn draw_legend(painter: &egui::Painter, rect: Rect) {
    let origin = rect.left_bottom() + Vec2::new(18.0, -18.0);
    painter.circle_filled(origin, 5.0, Color32::from_rgb(52, 211, 153));
    painter.text(
        origin + Vec2::new(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "start",
        egui::FontId::proportional(13.0),
        Color32::LIGHT_GRAY,
    );
    let exit = origin + Vec2::new(62.0, 0.0);
    painter.circle_filled(exit, 5.0, Color32::from_rgb(251, 113, 133));
    painter.text(
        exit + Vec2::new(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "exit",
        egui::FontId::proportional(13.0),
        Color32::LIGHT_GRAY,
    );
}
