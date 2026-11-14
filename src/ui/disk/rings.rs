//! GPUI element that paints the sunburst from a laid-out frame.
//!
//! One canvas draws the tracks, segments, selection outline and hub, including
//! its text. Text is painted by the canvas so it scales with the rings. Corners
//! are exact quarter circles, because lyon's round line joins are not exposed.

use std::cell::Cell;
use std::f32::consts::PI;
use std::rc::Rc;

use gpui_kit::{
    canvas, div, font, point, px, App, Bounds, ClickEvent, ElementId, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, MouseMoveEvent, ParentElement as _, PathBuilder, Pixels,
    Point, SharedString, StatefulInteractiveElement as _, Styled as _, TextAlign, TextRun, Window,
};

use super::geometry::{
    self, polar, Arc, DrawArc, HitIndex, Segment, CENTRE, DESIGN_SIZE, HUB_RADIUS, RING_RADII,
};
use crate::theme;

/// Opacity multiplier for segments that are not lit while something is hovered.
const DIM: f32 = 0.22;
/// Lightness multiplier for the hovered segment, a stand-in for the prototype's brightness filter.
const HOT_BRIGHTNESS: f32 = 1.18;
const OUTLINE_WIDTH: f32 = 1.6;
/// Hit testing ignores segments fainter than this, so fading ones cannot be clicked.
const HIT_MIN_OPACITY: f32 = 0.5;
/// Cubic handle length for a quarter circle, as a fraction of the radius.
const KAPPA: f32 = 0.552_284_8;

/// Everything the ring element draws. `frame` is the output of
/// `motion::Transition::sample`, and it can be empty.
pub struct RingsProps {
    /// Segments with their opacity, in paint order.
    pub frame: Vec<(Segment, f32)>,
    /// Category colour for each segment, already shaded by the caller.
    pub colors: Rc<dyn Fn(&Segment) -> Hsla>,
    /// Key under the pointer. Unlit segments dim to `DIM` while this is set.
    pub hovered: Option<u64>,
    /// Keys that stay lit while hovering.
    pub lit: Rc<dyn Fn(u64) -> bool>,
    /// Key whose outline is drawn. Pass `None` while the frame is tweening.
    pub selected: Option<u64>,
    /// Centred hub lines: size (large), name, and sub-line.
    pub hub_lines: [SharedString; 3],
    /// Small line above the hub, shown when zoomed in.
    pub hub_up_label: Option<SharedString>,
}

/// A click on the rings. `Segment` carries an index into `RingsProps::frame`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingClick {
    Segment(usize),
    Hub,
}

/// Renders the rings in a square centred in the parent. It scales to
/// `min(width, height) / DESIGN_SIZE`.
///
/// `on_hover` fires with the frame index under the pointer, or `None`, and
/// only when that index changes. `on_click` fires with a hub or segment click.
/// The cursor is a pointer while `hovered` is set, so callers should set it
/// from `on_hover`.
pub fn rings(
    id: impl Into<ElementId>,
    props: RingsProps,
    on_hover: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    on_click: impl Fn(RingClick, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let fit = Rc::new(Cell::new(Fit::default()));
    let index = Rc::new(HitIndex::new(
        props
            .frame
            .iter()
            .enumerate()
            .filter(|(_, (_, opacity))| *opacity >= HIT_MIN_OPACITY)
            .map(|(i, (segment, _))| (i, segment)),
    ));
    let last_hover = Rc::new(Cell::new(None::<usize>));
    let on_hover = Rc::new(on_hover);
    let pointer = props.hovered.is_some();

    let prepaint_fit = fit.clone();
    let paint_canvas = canvas(
        move |bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App| {
            prepaint_fit.set(Fit::of(bounds));
        },
        move |bounds: Bounds<Pixels>, (), window: &mut Window, cx: &mut App| {
            paint(&props, Fit::of(bounds), window, cx);
        },
    );

    let mut root = div()
        .id(id)
        .relative()
        .size_full()
        .child(paint_canvas.absolute().size_full());
    if pointer {
        root = root.cursor_pointer();
    }

    let move_fit = fit.clone();
    let move_index = index.clone();
    let move_last = last_hover.clone();
    let move_hover = on_hover.clone();
    let click_fit = fit;
    root.on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
        let next = move_fit
            .get()
            .design(event.position)
            .and_then(|(x, y)| geometry::hit_test(&move_index, x, y));
        if move_last.get() != next {
            move_last.set(next);
            move_hover(next, window, cx);
        }
    })
    .on_hover(move |inside: &bool, window, cx| {
        if !*inside && last_hover.replace(None).is_some() {
            on_hover(None, window, cx);
        }
    })
    .on_click(move |event: &ClickEvent, window, cx| {
        let Some((x, y)) = click_fit.get().design(event.position()) else {
            return;
        };
        if geometry::in_hub(x, y) {
            on_click(RingClick::Hub, window, cx);
        } else if let Some(i) = geometry::hit_test(&index, x, y) {
            on_click(RingClick::Segment(i), window, cx);
        }
    })
}

/// Maps design units to window pixels for the square that fits some bounds.
#[derive(Clone, Copy, Debug, Default)]
struct Fit {
    /// Top-left of the design square, in window pixels.
    x: f32,
    y: f32,
    /// Pixels per design unit. Zero until the element has been laid out.
    scale: f32,
}

impl Fit {
    fn of(bounds: Bounds<Pixels>) -> Self {
        let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
        let side = w.min(h);
        Self {
            x: bounds.origin.x.as_f32() + (w - side) / 2.,
            y: bounds.origin.y.as_f32() + (h - side) / 2.,
            scale: side / DESIGN_SIZE,
        }
    }

    /// Window point for a design point relative to the centre.
    fn point(&self, (x, y): (f32, f32)) -> Point<Pixels> {
        point(
            px(self.x + (CENTRE + x) * self.scale),
            px(self.y + (CENTRE + y) * self.scale),
        )
    }

    fn len(&self, design: f32) -> Pixels {
        px(design * self.scale)
    }

    /// Design point relative to the centre for a window point, or `None` before layout.
    fn design(&self, p: Point<Pixels>) -> Option<(f32, f32)> {
        (self.scale > 0.).then(|| {
            (
                (p.x.as_f32() - self.x) / self.scale - CENTRE,
                (p.y.as_f32() - self.y) / self.scale - CENTRE,
            )
        })
    }
}

fn paint(props: &RingsProps, fit: Fit, window: &mut Window, cx: &mut App) {
    for (inner, outer) in RING_RADII {
        let width = outer - inner - 2.;
        let mid = (inner + outer) / 2.;
        stroke(window, fit.len(width), theme::elev2(), |b| {
            circle(b, &fit, mid)
        });
    }

    for (segment, opacity) in &props.frame {
        if *opacity <= 0. {
            continue;
        }
        let Some(draw) = geometry::drawable(segment.arc) else {
            continue;
        };
        let dimmed = props.hovered.is_some() && !(props.lit)(segment.key);
        let hot = props.hovered == Some(segment.key);
        let mut colour = (props.colors)(segment);
        if hot {
            colour.l = (colour.l * HOT_BRIGHTNESS).min(1.);
        }
        colour.a *= opacity * if dimmed { DIM } else { 1. };
        fill(window, colour, |b| rounded(b, &fit, draw));
    }

    if let Some(key) = props.selected {
        let selected = props
            .frame
            .iter()
            .find(|(segment, opacity)| segment.key == key && *opacity >= 0.99)
            .and_then(|(segment, _)| geometry::outline(segment.arc));
        if let Some(arc) = selected {
            stroke(window, fit.len(OUTLINE_WIDTH), theme::text(), |b| {
                sector(b, &fit, arc)
            });
        }
    }

    fill(window, theme::elev2(), |b| circle(b, &fit, HUB_RADIUS));
    stroke(window, fit.len(1.), theme::border(), |b| {
        circle(b, &fit, HUB_RADIUS)
    });
    paint_hub(props, &fit, window, cx);
}

fn fill(window: &mut Window, colour: Hsla, build: impl FnOnce(&mut PathBuilder)) {
    let mut builder = PathBuilder::fill();
    build(&mut builder);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

fn stroke(window: &mut Window, width: Pixels, colour: Hsla, build: impl FnOnce(&mut PathBuilder)) {
    let mut builder = PathBuilder::stroke(width);
    build(&mut builder);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

fn radius(fit: &Fit, design: f32) -> Point<Pixels> {
    point(fit.len(design), fit.len(design))
}

/// Plain annular sector, as used for the selection outline.
fn sector(b: &mut PathBuilder, fit: &Fit, arc: Arc) {
    let large = arc.a1 - arc.a0 > PI;
    b.move_to(fit.point(polar(arc.r1, arc.a0)));
    b.arc_to(
        radius(fit, arc.r1),
        px(0.),
        large,
        true,
        fit.point(polar(arc.r1, arc.a1)),
    );
    b.line_to(fit.point(polar(arc.r0, arc.a1)));
    b.arc_to(
        radius(fit, arc.r0),
        px(0.),
        large,
        false,
        fit.point(polar(arc.r0, arc.a0)),
    );
    b.close();
}

/// Full circle of radius `r`, built from two half arcs.
fn circle(b: &mut PathBuilder, fit: &Fit, r: f32) {
    let top = polar(r, 0.);
    let bottom = polar(r, PI);
    b.move_to(fit.point(top));
    b.arc_to(radius(fit, r), px(0.), false, true, fit.point(bottom));
    b.arc_to(radius(fit, r), px(0.), false, true, fit.point(top));
    b.close();
}

/// The sector dilated by its corner radius, so every corner is a quarter circle.
/// The straight edges are offset by the corner radius, and the arcs sit at
/// `r1 + corner` and `r0 - corner`.
fn rounded(b: &mut PathBuilder, fit: &Fit, draw: DrawArc) {
    let DrawArc {
        a0,
        a1,
        r0,
        r1,
        corner: c,
    } = draw;
    let outer = r1 + c;
    let inner = r0 - c;
    let (t0, t1) = (tangent(a0), tangent(a1));
    let (n0, n1) = (radial(a0), radial(a1));
    let large = a1 - a0 > PI;

    // Outer-start corner: from the start edge's offset line onto the outer arc.
    let start = offset(polar(r1, a0), t0, -c);
    b.move_to(fit.point(start));
    corner(b, fit, start, polar(outer, a0), n0, t0, c);

    b.arc_to(
        radius(fit, outer),
        px(0.),
        large,
        true,
        fit.point(polar(outer, a1)),
    );

    // Outer-end corner, then the end edge inward to the inner-end corner.
    let outer_end = polar(outer, a1);
    let end_outer = offset(polar(r1, a1), t1, c);
    corner(b, fit, outer_end, end_outer, t1, scale(n1, -1.), c);
    b.line_to(fit.point(offset(polar(r0, a1), t1, c)));

    let end_inner = polar(inner, a1);
    corner(
        b,
        fit,
        offset(polar(r0, a1), t1, c),
        end_inner,
        scale(n1, -1.),
        scale(t1, -1.),
        c,
    );

    b.arc_to(
        radius(fit, inner),
        px(0.),
        large,
        false,
        fit.point(polar(inner, a0)),
    );

    // Inner-start corner, then the start edge back out to the first point.
    let inner_start = polar(inner, a0);
    let end_start = offset(polar(r0, a0), t0, -c);
    corner(b, fit, inner_start, end_start, scale(t0, -1.), n0, c);
    b.close();
}

/// Quarter-circle cubic from `from` to `to`. The tangents are the unit
/// directions of travel at each end. Going around the corner needs no sweep
/// flag, so it cannot pick the wrong side.
fn corner(
    b: &mut PathBuilder,
    fit: &Fit,
    from: (f32, f32),
    to: (f32, f32),
    tangent_from: (f32, f32),
    tangent_to: (f32, f32),
    c: f32,
) {
    let handle = KAPPA * c;
    let control_a = offset(from, tangent_from, handle);
    let control_b = offset(to, tangent_to, -handle);
    b.cubic_bezier_to(fit.point(to), fit.point(control_a), fit.point(control_b));
}

/// Unit vector of increasing angle at `a`, along the arc.
fn tangent(a: f32) -> (f32, f32) {
    (a.cos(), a.sin())
}

/// Unit vector pointing outward from the centre at `a`.
fn radial(a: f32) -> (f32, f32) {
    polar(1., a)
}

fn offset(p: (f32, f32), v: (f32, f32), k: f32) -> (f32, f32) {
    (p.0 + k * v.0, p.1 + k * v.1)
}

fn scale(v: (f32, f32), k: f32) -> (f32, f32) {
    (v.0 * k, v.1 * k)
}

/// One centred line of hub text. `size` is in design units.
struct HubLine {
    text: SharedString,
    size: f32,
    weight: FontWeight,
    colour: Hsla,
}

fn paint_hub(props: &RingsProps, fit: &Fit, window: &mut Window, cx: &mut App) {
    let mut lines: Vec<HubLine> = Vec::new();
    if let Some(up) = &props.hub_up_label {
        lines.push(HubLine {
            text: up.clone(),
            size: 12.,
            weight: FontWeight::MEDIUM,
            colour: theme::text_faint(),
        });
    }
    lines.push(HubLine {
        text: props.hub_lines[0].clone(),
        size: 32.,
        weight: FontWeight::SEMIBOLD,
        colour: theme::text(),
    });
    lines.push(HubLine {
        text: props.hub_lines[1].clone(),
        size: 14.,
        weight: FontWeight::MEDIUM,
        colour: theme::text_dim(),
    });
    lines.push(HubLine {
        text: props.hub_lines[2].clone(),
        size: 12.,
        weight: FontWeight::NORMAL,
        colour: theme::text_faint(),
    });

    const GAP: f32 = 4.;
    let heights: Vec<f32> = lines.iter().map(|line| line.size * 1.3).collect();
    let total: f32 = heights.iter().sum::<f32>() + GAP * (lines.len() - 1) as f32;
    let width = HUB_RADIUS * 1.6;
    let mut top = -total / 2.;
    for (line, height) in lines.iter().zip(heights) {
        if !line.text.is_empty() {
            paint_line(window, cx, fit, line, top, height, width);
        }
        top += height + GAP;
    }
}

/// Paints one hub line centred horizontally, with its box starting at `top`.
fn paint_line(
    window: &mut Window,
    cx: &mut App,
    fit: &Fit,
    line: &HubLine,
    top: f32,
    height: f32,
    width: f32,
) {
    let mut family = font(theme::FONT_FAMILY);
    family.weight = line.weight;
    let run = TextRun {
        len: line.text.len(),
        font: family,
        color: line.colour,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped =
        window
            .text_system()
            .shape_line(line.text.clone(), px(line.size * fit.scale), &[run], None);
    let origin = fit.point((-width / 2., top));
    let _ = shaped.paint(
        origin,
        px(height * fit.scale),
        TextAlign::Center,
        Some(px(width * fit.scale)),
        window,
        cx,
    );
}
