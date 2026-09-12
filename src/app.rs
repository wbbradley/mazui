use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use rand::Rng;

use crate::geometry::{Point, closest_point_on_segment};
use crate::maze::{Maze, MazeConfig, generate};

const TUBE_WIDTH_TO_UNIT: f32 = 0.75;
const MARKER_RADIUS_TO_TUBE_WIDTH: f32 = 0.3;
const VISIBLE_BALL_DIAMETERS: f32 = 20.0;
const FOLLOW_SPRING: f32 = 14.0;
const VELOCITY_DAMPING: f32 = 2.8;
const WALL_RESTITUTION: f32 = 0.45;

pub struct MazeApp {
    config: MazeConfig,
    maze: Option<Maze>,
    result_receiver: Option<Receiver<Maze>>,
    generating: bool,
    ball_position: Option<Point>,
    ball_velocity: Point,
}

impl MazeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = Self {
            config: MazeConfig::default(),
            maze: None,
            result_receiver: None,
            generating: false,
            ball_position: None,
            ball_velocity: Point::ZERO,
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
            self.ball_position = Some(maze.nodes[maze.start]);
            self.ball_velocity = Point::ZERO;
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
        ui.add(egui::Slider::new(&mut self.config.unit, 5.0..=30.0).text("generation unit"));
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

    fn draw_maze(&mut self, ui: &mut egui::Ui) {
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
        let full_maze_scale = (canvas.width() / diameter).min(canvas.height() / diameter);
        let ball_diameter_world =
            maze.unit * TUBE_WIDTH_TO_UNIT * MARKER_RADIUS_TO_TUBE_WIDTH * 2.0;
        let focused_scale = canvas.width() / (VISIBLE_BALL_DIAMETERS * ball_diameter_world);
        let scale = focused_scale.max(full_maze_scale);
        // Centerlines are generated at least one unit apart. Keeping the full
        // tube width below one scaled unit guarantees separated tubes remain
        // visually disjoint at every zoom level.
        let path_width = (maze.unit * scale * TUBE_WIDTH_TO_UNIT).max(1.0);
        let marker_radius = path_width * MARKER_RADIUS_TO_TUBE_WIDTH;
        let ball_position = self.ball_position.get_or_insert(maze.nodes[maze.start]);
        let camera_before_physics = *ball_position;
        let to_world = |position: Pos2| -> Point {
            Point::new(
                camera_before_physics.x + (position.x - canvas.center().x) / scale,
                camera_before_physics.y + (canvas.center().y - position.y) / scale,
            )
        };

        let cursor_target = response.hover_pos().map(to_world);
        let dt = ui.input(|input| input.stable_dt).min(0.05);
        advance_ball(
            ball_position,
            &mut self.ball_velocity,
            cursor_target,
            maze,
            dt,
        );
        let camera_position = *ball_position;
        let to_screen = |point: Point| -> Pos2 {
            Pos2::new(
                canvas.center().x + (point.x - camera_position.x) * scale,
                canvas.center().y - (point.y - camera_position.y) * scale,
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
                Stroke::new(path_width, path_color),
            );
        }
        // egui line segments have flat caps. A half-stroke disc at every vector
        // node creates round end caps and smooth joins at corners and branches.
        for &node in &maze.nodes {
            painter.circle_filled(to_screen(node), path_width * 0.5, path_color);
        }

        painter.circle_filled(
            to_screen(*ball_position),
            marker_radius,
            Color32::from_rgb(52, 211, 153),
        );
        let exit_position = to_screen(maze.nodes[maze.end]);
        let fully_visible = response.rect.shrink(marker_radius);
        let exit_marker_position = if fully_visible.contains(exit_position) {
            exit_position
        } else {
            let indicator_bounds = response.rect.shrink(marker_radius + 12.0);
            ray_to_rect_edge(canvas.center(), exit_position, indicator_bounds)
        };
        painter.circle_filled(
            exit_marker_position,
            marker_radius,
            Color32::from_rgb(251, 113, 133),
        );

        draw_legend(&painter, response.rect);
    }
}

impl eframe::App for MazeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive_result();
        if self.maze.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        } else if self.generating {
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

fn advance_ball(
    position: &mut Point,
    velocity: &mut Point,
    cursor_target: Option<Point>,
    maze: &Maze,
    dt: f32,
) {
    let tube_radius = maze.unit * TUBE_WIDTH_TO_UNIT * 0.5;
    let ball_radius = maze.unit * TUBE_WIDTH_TO_UNIT * MARKER_RADIUS_TO_TUBE_WIDTH;
    let allowed_offset = tube_radius - ball_radius;
    let substeps = (dt / (1.0 / 120.0)).ceil().clamp(1.0, 8.0) as usize;
    let step_dt = dt / substeps as f32;

    for _ in 0..substeps {
        if let Some(target) = cursor_target {
            *velocity = *velocity + (target - *position) * (FOLLOW_SPRING * step_dt);
        }
        *velocity = *velocity * (-VELOCITY_DAMPING * step_dt).exp();

        let max_speed = maze.unit * 20.0;
        let speed_squared = velocity.length_squared();
        if speed_squared > max_speed * max_speed {
            *velocity = *velocity * (max_speed / speed_squared.sqrt());
        }
        *position = *position + *velocity * step_dt;

        let Some(closest) = nearest_maze_point(*position, maze) else {
            *position = maze.nodes[maze.start];
            *velocity = Point::ZERO;
            return;
        };
        let offset = *position - closest;
        let distance = offset.length_squared().sqrt();
        if distance > allowed_offset {
            let normal = offset * (1.0 / distance);
            *position = closest + normal * allowed_offset;
            let outward_speed = velocity.dot(normal);
            if outward_speed > 0.0 {
                *velocity = *velocity - normal * ((1.0 + WALL_RESTITUTION) * outward_speed);
            }
        }
    }
}

fn nearest_maze_point(position: Point, maze: &Maze) -> Option<Point> {
    maze.edges
        .iter()
        .map(|&(a, b)| closest_point_on_segment(position, maze.nodes[a], maze.nodes[b]))
        .min_by(|a, b| position.distance(*a).total_cmp(&position.distance(*b)))
}

fn ray_to_rect_edge(origin: Pos2, target: Pos2, bounds: Rect) -> Pos2 {
    if bounds.contains(target) {
        return target;
    }

    let direction = target - origin;
    if direction.length_sq() < 1e-12 {
        return origin;
    }

    let horizontal_t = if direction.x > 0.0 {
        (bounds.right() - origin.x) / direction.x
    } else if direction.x < 0.0 {
        (bounds.left() - origin.x) / direction.x
    } else {
        f32::INFINITY
    };
    let vertical_t = if direction.y > 0.0 {
        (bounds.bottom() - origin.y) / direction.y
    } else if direction.y < 0.0 {
        (bounds.top() - origin.y) / direction.y
    } else {
        f32::INFINITY
    };
    origin + direction * horizontal_t.min(vertical_t).max(0.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_ball_remains_inside_tube() {
        let maze = Maze {
            boundary: Vec::new(),
            nodes: vec![Point::ZERO, Point::new(100.0, 0.0)],
            edges: vec![(0, 1)],
            unit: 20.0,
            start: 0,
            end: 1,
            solution_units: 1,
            attempts: 1,
        };
        let mut position = Point::new(50.0, 0.0);
        let mut velocity = Point::ZERO;
        for _ in 0..120 {
            advance_ball(
                &mut position,
                &mut velocity,
                Some(Point::new(50.0, 100.0)),
                &maze,
                1.0 / 60.0,
            );
        }

        let tube_radius = maze.unit * TUBE_WIDTH_TO_UNIT * 0.5;
        let ball_radius = maze.unit * TUBE_WIDTH_TO_UNIT * MARKER_RADIUS_TO_TUBE_WIDTH;
        assert!(position.y.abs() <= tube_radius - ball_radius + 1e-4);
        assert!(velocity.y <= 0.0);
    }

    #[test]
    fn exit_indicator_preserves_direction_at_viewport_edge() {
        let bounds = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(90.0, 90.0));
        let position = ray_to_rect_edge(Pos2::new(50.0, 50.0), Pos2::new(150.0, 100.0), bounds);
        assert_eq!(position, Pos2::new(90.0, 70.0));
    }

    #[test]
    fn visible_exit_indicator_uses_true_position() {
        let bounds = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(90.0, 90.0));
        let target = Pos2::new(20.0, 30.0);
        assert_eq!(
            ray_to_rect_edge(Pos2::new(50.0, 50.0), target, bounds),
            target
        );
    }
}
