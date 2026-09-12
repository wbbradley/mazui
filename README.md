# Mazui

Mazui is a deterministic, native Rust maze generator. It grows a branching path
network inside a regular polygon, with every random choice derived from one seed.

## Run

```sh
cargo run --release
```

The right-hand control pane exposes the polygon and generation parameters. Click
**Randomized** to choose a new seed, then **Generate maze**. Generation runs on a
worker thread, so the interface remains responsive.

Move the pointer over the maze to pull the green start ball toward it. The ball
has damped momentum, bounces softly at walls, and remains fully inside the white
maze tubes. The view follows the ball and zooms to approximately 20 ball
diameters across, with the full-polygon view retained as the zoom-out limit.

## Model

- Directions are restricted to 15-degree increments.
- New line lengths are integer multiples of the configured unit.
- Every unit point becomes a future branching candidate.
- The initial point is sampled uniformly from the usable polygon interior.
- New lines remain at least one unit from existing linework, except at their
  intentional branch point.
- Rendered tube width is proportional to the generation unit and remains below
  the guaranteed centerline clearance, preventing visual overlap at small units.
- Generation ends after the configured number of consecutive failed extensions.
- The start and exit are chosen as the endpoints of the network's graph diameter:
  the exact longest shortest path through this tree-shaped maze. Because the
  network is a tree, two breadth-first traversals find this in linear time.

The maze is retained as vector points and edges rather than a bitmap. That model
can later be mapped directly to SVG/PDF and physical page dimensions for printing.
