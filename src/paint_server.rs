// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::render::prelude::*;

pub fn fill(
    tree: &usvg::Tree,
    fill: &Option<usvg::Fill>,
    bbox: Rect,
    global_ts: usvg::Transform,
) -> skia::Paint {
    let mut paint = skia::Paint::new();
    paint.set_style(skia::PaintStyle::Fill);

    if let Some(ref fill) = fill {
        let opacity = fill.opacity;
        match fill.paint {
            usvg::Paint::Color(c) => {
                paint.set_color(c.red, c.green, c.blue, opacity.to_u8());
            }
            usvg::Paint::Link(ref id) => {
                if let Some(node) = tree.defs_by_id(id) {
                    match *node.borrow() {
                        usvg::NodeKind::LinearGradient(ref lg) => {
                            prepare_linear(lg, opacity, bbox, &mut paint);
                        }
                        usvg::NodeKind::RadialGradient(ref rg) => {
                            prepare_radial(rg, opacity, bbox, &mut paint);
                        }
                        usvg::NodeKind::Pattern(ref pattern) => {
                            prepare_pattern(&node, pattern, global_ts, bbox, opacity, &mut paint);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    paint
}

pub fn stroke(
    tree: &usvg::Tree,
    stroke: &Option<usvg::Stroke>,
    bbox: Rect,
    global_ts: usvg::Transform,
) -> skia::Paint {
    let mut paint = skia::Paint::new();
    paint.set_style(skia::PaintStyle::Stroke);

    if let Some(ref stroke) = stroke {
        let opacity = stroke.opacity;
        match stroke.paint {
            usvg::Paint::Color(c) => {
                paint.set_color(c.red, c.green, c.blue, opacity.to_u8());
            }
            usvg::Paint::Link(ref id) => {
                if let Some(node) = tree.defs_by_id(id) {
                    match *node.borrow() {
                        usvg::NodeKind::LinearGradient(ref lg) => {
                            prepare_linear(lg, opacity, bbox, &mut paint);
                        }
                        usvg::NodeKind::RadialGradient(ref rg) => {
                            prepare_radial(rg, opacity, bbox, &mut paint);
                        }
                        usvg::NodeKind::Pattern(ref pattern) => {
                            prepare_pattern(&node, pattern, global_ts, bbox, opacity, &mut paint);
                        }
                        _ => {}
                    }
                }
            }
        }

        let stroke_cap = match stroke.linecap {
            usvg::LineCap::Butt => skia::StrokeCap::Butt,
            usvg::LineCap::Round => skia::StrokeCap::Round,
            usvg::LineCap::Square => skia::StrokeCap::Square,
        };
        paint.set_stroke_cap(stroke_cap);

        let stroke_join = match stroke.linejoin {
            usvg::LineJoin::Miter => skia::StrokeJoin::Miter,
            usvg::LineJoin::Round => skia::StrokeJoin::Round,
            usvg::LineJoin::Bevel => skia::StrokeJoin::Bevel,
        };
        paint.set_stroke_join(stroke_join);

        paint.set_stroke_miter(stroke.miterlimit.value() as f32);
        paint.set_stroke_width(stroke.width.value() as f32);

        if let Some(ref list) = stroke.dasharray {
            let list: Vec<_> = list.iter().map(|n| *n as f32).collect();
            let path_effect = skia::PathEffect::new_dash_path(&list, stroke.dashoffset).unwrap();
            paint.set_path_effect(&path_effect);
        }
    }

    paint
}

fn prepare_linear(
    g: &usvg::LinearGradient,
    opacity: usvg::Opacity,
    bbox: Rect,
    paint: &mut skia::Paint,
) {
    let gradient = skia::LinearGradient {
        start_point: (g.x1 as f32, g.y1 as f32),
        end_point: (g.x2 as f32, g.y2 as f32),
        base: prepare_base_gradient(g, opacity, &bbox)
    };

    let shader = skia::Shader::new_linear_gradient(&gradient).unwrap();
    paint.set_shader(&shader);
}

fn prepare_radial(
    g: &usvg::RadialGradient,
    opacity: usvg::Opacity,
    bbox: Rect,
    paint: &mut skia::Paint,
) {

    let gradient = skia::TwoPointConicalGradient {
        start: (g.fx as f32, g.fy as f32),
        start_radius: 0.0,
        end: (g.cx as f32, g.cy as f32),
        end_radius: g.r.value() as f32,
        base: prepare_base_gradient(g, opacity, &bbox)
    };

    let shader = skia::Shader::new_two_point_conical_gradient(&gradient).unwrap();
    paint.set_shader(&shader);
}

fn prepare_base_gradient(
    g: &usvg::BaseGradient,
    opacity: usvg::Opacity,
    bbox: &Rect
) -> skia::Gradient {

    let tile_mode = match g.spread_method {
        usvg::SpreadMethod::Pad => skia::TileMode::Clamp,
        usvg::SpreadMethod::Reflect => skia::TileMode::Mirror,
        usvg::SpreadMethod::Repeat => skia::TileMode::Repeat,
    };

    let transform = {
        if g.units == usvg::Units::ObjectBoundingBox {
            let mut ts = usvg::Transform::from_bbox(*bbox);
            ts.append(&g.transform);
            ts.to_native()
        } else {
            g.transform.to_native()
        }
    };

    let mut colors = Vec::new();
    let mut positions = Vec::new();

    for stop in &g.stops {
        let a = stop.opacity * opacity;
        let color = skia::Color::new(a.to_u8(), stop.color.red, stop.color.green, stop.color.blue);
        colors.push(color);
        positions.push(stop.offset.value() as f32);
    }

    skia::Gradient { colors, positions, tile_mode, transform }
}

fn prepare_pattern(
    pattern_node: &usvg::Node,
    pattern: &usvg::Pattern,
    global_ts: usvg::Transform,
    bbox: Rect,
    opacity: usvg::Opacity,
    paint: &mut skia::Paint,
) {
    let r = if pattern.units == usvg::Units::ObjectBoundingBox {
        pattern.rect.bbox_transform(bbox)
    } else {
        pattern.rect
    };

    let (sx, sy) = global_ts.get_scale();

    let img_size = try_opt!(Size::new(r.width() * sx, r.height() * sy)).to_screen_size();
    let mut surface = try_opt!(crate::render::create_subsurface(img_size));
    surface.clear();

    surface.scale(sx as f32, sy as f32);
    if let Some(vbox) = pattern.view_box {
        let ts = usvg::utils::view_box_to_transform(vbox.rect, vbox.aspect, r.size());
        surface.concat(ts.to_native());
    } else if pattern.content_units == usvg::Units::ObjectBoundingBox {
        // 'Note that this attribute has no effect if attribute `viewBox` is specified.'

        // We don't use Transform::from_bbox(bbox) because `x` and `y` should be
        // ignored for some reasons...
        surface.scale(bbox.width() as f32, bbox.height() as f32);
    }

    let mut layers = Layers::new(img_size);
    crate::render::render_group(pattern_node, &mut RenderState::Ok, &mut layers, &mut surface);

    let mut ts = usvg::Transform::default();
    ts.append(&pattern.transform);
    ts.translate(r.x(), r.y());
    ts.scale(1.0 / sx, 1.0 / sy);
    let shader = skia::Shader::new_from_surface_image(&surface, ts.to_native()).unwrap();
    paint.set_shader(&shader);

    if !opacity.is_default() {
        paint.set_alpha(opacity.to_u8());
    };
}
