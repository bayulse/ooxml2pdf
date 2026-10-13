//! DrawingML outer effects; backend-independent owned semantics.
use crate::{
    PptxError, Theme,
    theme::ColorValue,
    theme_xml::{Context, effect_color_value},
};
use o2p_core::{Color, Point, Shadow, Transform};
use roxmltree::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OuterShadow {
    blur: u32,
    distance: u32,
    direction: u32,
    color: ColorValue,
    rotate: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedShadow {
    pub shadow: Shadow,
    pub rotate: bool,
}

/// DrawingML positive coordinates are EMUs, angles are clockwise 1/60000 degree.
fn coordinate(value: u32) -> f32 {
    crate::emu_to_points(i64::from(value))
}
fn offset(distance: u32, direction: u32) -> Point {
    let angle = f64::from(crate::drawingml_rotation_to_degrees(direction as i32));
    let (sin, cos) = angle.to_radians().sin_cos();
    let distance = f64::from(coordinate(distance));
    Point {
        x: (distance * cos) as f32,
        y: (distance * sin) as f32,
    }
}
impl OuterShadow {
    pub(crate) fn resolve(
        &self,
        theme: Option<&Theme>,
        placeholder: Option<Color>,
        context: &mut Context<'_>,
    ) -> Option<ResolvedShadow> {
        Some(ResolvedShadow {
            shadow: Shadow {
                offset: offset(self.distance, self.direction),
                blur_radius: coordinate(self.blur),
                color: self.color.resolve(theme, placeholder, context)?,
                opacity: self.color.opacity as f32 / 100_000.0,
            },
            rotate: self.rotate,
        })
    }
}
impl ResolvedShadow {
    pub(crate) fn local(mut self, transform: Transform) -> Shadow {
        if !self.rotate {
            // Element transforms are orthogonal rotations/reflections. Keep offset page-relative.
            let p = self.shadow.offset;
            self.shadow.offset = Point {
                x: transform.a * p.x + transform.b * p.y,
                y: transform.c * p.x + transform.d * p.y,
            };
        }
        self.shadow
    }
}
fn number(
    node: Node<'_, '_>,
    name: &str,
    default: u32,
    context: &Context<'_>,
) -> Result<u32, PptxError> {
    node.attribute(name).map_or(Ok(default), |s| {
        s.trim()
            .parse()
            .map_err(|_| context.invalid(format!("invalid outer shadow {name}")))
    })
}
/// None = absent; Some(None) = explicit empty/unsupported effect list, suppressing parents.
pub(crate) fn list(
    parent: Node<'_, '_>,
    ns: &str,
    context: &mut Context<'_>,
) -> Result<Option<Option<OuterShadow>>, PptxError> {
    let mut lists = parent
        .children()
        .filter(|n| n.has_tag_name((ns, "effectLst")));
    let Some(list) = lists.next() else {
        // A direct effect DAG is also an explicit effect choice; never restore theme effects.
        if parent.children().any(|n| n.has_tag_name((ns, "effectDag"))) {
            return Ok(Some(None));
        }
        return Ok(None);
    };
    if parent.children().any(|n| n.has_tag_name((ns, "effectDag"))) {
        return Err(context.invalid("multiple effect containers"));
    }
    if lists.next().is_some() {
        return Err(context.invalid("duplicate effect list"));
    }
    context.attributes(list, &[]);
    context.element_only(list)?;
    context.other_children(list, ns, &["outerShdw"]);
    let mut shadows = list
        .children()
        .filter(|n| n.has_tag_name((ns, "outerShdw")));
    let Some(node) = shadows.next() else {
        return Ok(Some(None));
    };
    if shadows.next().is_some() {
        return Err(context.invalid("duplicate outer shadow"));
    }
    context.attributes(
        node,
        &[
            "blurRad",
            "dist",
            "dir",
            "sx",
            "sy",
            "kx",
            "ky",
            "algn",
            "rotWithShape",
        ],
    );
    context.element_only(node)?;
    let blur = number(node, "blurRad", 0, context)?;
    let distance = number(node, "dist", 0, context)?;
    let direction = number(node, "dir", 0, context)?;
    if direction >= 21_600_000 {
        return Err(context.invalid("out-of-range outer shadow direction"));
    }
    if let Some(alignment) = node.attribute("algn") {
        if !matches!(
            alignment,
            "tl" | "t" | "tr" | "l" | "ctr" | "r" | "bl" | "b" | "br"
        ) {
            return Err(context.invalid("invalid shadow alignment"));
        }
    }
    let rotate = match node.attribute("rotWithShape").unwrap_or("true") {
        "0" | "false" => false,
        "1" | "true" => true,
        _ => return Err(context.invalid("invalid shadow rotation boolean")),
    };
    for (name, default) in [("sx", "100000"), ("sy", "100000"), ("kx", "0"), ("ky", "0")] {
        if node.attribute(name).is_some_and(|v| v != default) {
            context.warn(format!(
                "outerShdw {name}: scaled/skewed shadows unsupported; effect omitted"
            ));
            return Ok(Some(None));
        }
    }
    let mut colors = node.children().filter(|n| n.is_element());
    let color = colors
        .next()
        .ok_or_else(|| context.invalid("shadow missing color"))?;
    if colors.next().is_some() {
        return Err(context.invalid("shadow has multiple colors"));
    }
    let color = effect_color_value(color, ns, context)?;
    Ok(Some(Some(OuterShadow {
        blur,
        distance,
        direction,
        color,
        rotate,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_and_directions() {
        assert_eq!(coordinate(914400), 72.0);
        for (angle, x, y) in [
            (0, 72.0, 0.0),
            (5400000, 0.0, 72.0),
            (10800000, -72.0, 0.0),
            (16200000, 0.0, -72.0),
        ] {
            let p = offset(914400, angle);
            assert!((p.x - x).abs() < 0.0001 && (p.y - y).abs() < 0.0001);
        }
        assert_eq!(coordinate(40000), 40000.0 / 12700.0);
    }
}
