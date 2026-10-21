//! Line charts, drawn as SVG on the server: no script needed to see them.

use maud::{Markup, html};

use crate::fold::Series;
use crate::headline::shown;

const WIDTH: f64 = 760.0;
const HEIGHT: f64 = 240.0;
/// Room for the axes' labels: left, right, top, bottom.
const MARGIN: [f64; 4] = [64.0, 16.0, 12.0, 28.0];
const TICKS: usize = 5;
/// Charts with more points than this draw no dot for each.
const MOST_DOTS: usize = 400;
/// Colours for the lines, in turn.
const PALETTE: [&str; 8] = [
    "#c0392b", "#2471a3", "#229954", "#b9770e", "#7d3c98", "#17a589", "#ca6f1e", "#566573",
];

/// What a chart's values span.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    x: (f64, f64),
    y: (f64, f64),
}

impl Frame {
    /// Spanning every point of every series, a little more where all the
    /// values are one.
    fn around<'a>(series: impl Iterator<Item = &'a Series>) -> Option<Self> {
        let mut points = series.flatten().peekable();
        let &(x, y) = *points.peek()?;
        let mut frame = Self {
            x: (x, x),
            y: (y, y),
        };
        for &(x, y) in points {
            frame.x = (frame.x.0.min(x), frame.x.1.max(x));
            frame.y = (frame.y.0.min(y), frame.y.1.max(y));
        }
        for span in [&mut frame.x, &mut frame.y] {
            if span.0 == span.1 {
                *span = (span.0 - 1.0, span.1 + 1.0);
            }
        }
        Some(frame)
    }

    /// Where a point is drawn, in the SVG's own units: y grows downward.
    fn place(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let [left, right, top, bottom] = MARGIN;
        let across = (x - self.x.0) / (self.x.1 - self.x.0);
        let up = (y - self.y.0) / (self.y.1 - self.y.0);
        (
            left + across * (WIDTH - left - right),
            HEIGHT - bottom - up * (HEIGHT - top - bottom),
        )
    }
}

/// `count` evenly spaced values from `from` to `to`.
fn ticks((from, to): (f64, f64), count: usize) -> impl Iterator<Item = f64> {
    (0..count).map(move |n| from + (to - from) * n as f64 / (count - 1) as f64)
}

/// A chart of each named series over `x_label`, or nothing if they're all
/// empty.
pub fn chart(title: &str, x_label: &str, series: &[(String, &Series)]) -> Markup {
    let Some(frame) = Frame::around(series.iter().map(|(_, series)| *series)) else {
        return html! {};
    };
    let [left, right, _, bottom] = MARGIN;
    let dots = series.iter().map(|(_, s)| s.len()).sum::<usize>() <= MOST_DOTS;
    html! {
        figure.chart {
            figcaption { (title) span.faded { ", by " (x_label) } }
            svg viewBox=(format!("0 0 {WIDTH} {HEIGHT}")) role="img" aria-label=(title) {
                @for y in ticks(frame.y, TICKS) {
                    @let (_, at) = frame.place((frame.x.0, y));
                    line.grid x1=(left) x2=(WIDTH - right) y1=(at) y2=(at) {}
                    text.axis x=(left - 6.0) y=(at + 4.0) text-anchor="end" { (shown_axis(y, frame.y)) }
                }
                @for x in ticks(frame.x, TICKS + 1) {
                    @let (at, _) = frame.place((x, frame.y.0));
                    text.axis x=(at) y=(HEIGHT - bottom + 18.0) text-anchor="middle" { (shown_axis(x, frame.x)) }
                }
                @for (index, (name, points)) in series.iter().enumerate() {
                    @let colour = PALETTE[index % PALETTE.len()];
                    @let placed: Vec<(f64, f64)> = points.iter().map(|&point| frame.place(point)).collect();
                    polyline fill="none" stroke=(colour) stroke-width="2" points=(polyline(&placed)) {
                        title { (name) }
                    }
                    @if dots {
                        @for (&(x, y), &(at_x, at_y)) in points.iter().zip(&placed) {
                            circle cx=(at_x) cy=(at_y) r="2.5" fill=(colour) {
                                title { (name) ", " (x_label) " " (shown(x)) ": " (shown(y)) }
                            }
                        }
                    }
                }
            }
            @if series.len() > 1 {
                ul.legend {
                    @for (index, (name, _)) in series.iter().enumerate() {
                        li { span.swatch style=(format!("background:{}", PALETTE[index % PALETTE.len()])) {} (name) }
                    }
                }
            }
        }
    }
}

/// An axis label, to as many places as ticks across `span` need to differ.
fn shown_axis(value: f64, (from, to): (f64, f64)) -> String {
    let places = match to - from {
        span if span >= 50.0 => 0,
        span if span >= 5.0 => 1,
        _ => 2,
    };
    format!("{value:.places$}")
}

fn polyline(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lowest_and_highest_points_land_on_the_corners_of_the_plot() {
        let series: Series = vec![(0.0, 10.0), (5.0, 30.0), (10.0, 20.0)];
        let frame = Frame::around([&series].into_iter()).unwrap();
        let [left, right, top, bottom] = MARGIN;
        assert_eq!(frame.place((0.0, 10.0)), (left, HEIGHT - bottom));
        assert_eq!(frame.place((10.0, 30.0)), (WIDTH - right, top));
        assert_eq!(
            frame.place((5.0, 20.0)),
            ((left + WIDTH - right) / 2.0, (top + HEIGHT - bottom) / 2.0)
        );
    }

    #[test]
    fn a_flat_series_still_has_room_and_no_series_has_no_chart() {
        let flat: Series = vec![(1.0, 5.0), (2.0, 5.0)];
        let frame = Frame::around([&flat].into_iter()).unwrap();
        assert_eq!(frame.y, (4.0, 6.0));
        assert!(chart("Nothing", "day", &[]).into_string().is_empty());
        let drawn = chart("Purse", "day", &[("purse".into(), &flat)]).into_string();
        assert!(drawn.contains("<polyline"));
        assert!(drawn.contains("purse, day 1: 5"));
        assert!(
            drawn.contains(">4.50<"),
            "a narrow axis shows enough places"
        );
    }
}
