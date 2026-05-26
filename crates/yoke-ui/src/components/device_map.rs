//! SVG sketch of the `QuadStick` showing the eight clickable stations.
//!
//! The selected station is held in an `RwSignal<Option<String>>` owned by
//! [`EditorView`](crate::views::editor::EditorView); clicking a node writes
//! the station id into that signal so the bindings panel can filter on it.

use leptos::prelude::*;

use super::stations::{STATIONS, StationDef, StationKind};

#[component]
pub fn DeviceMap(selected: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        <svg class="qs-map" viewBox="0 0 280 220" xmlns="http://www.w3.org/2000/svg">
            <Region kind=StationKind::Joystick label="JOYSTICK" x=100.0 y=14.0 w=80.0 h=56.0/>
            <Region kind=StationKind::Mouthpiece label="MOUTHPIECE" x=20.0 y=90.0 w=180.0 h=40.0/>
            <Region kind=StationKind::Lip label="LIP" x=100.0 y=150.0 w=80.0 h=44.0/>
            <Region kind=StationKind::Side label="SIDE TUBE" x=220.0 y=90.0 w=44.0 h=60.0/>

            <For
                each=move || STATIONS.iter().copied()
                key=|s: &StationDef| s.id
                children=move |s: StationDef| {
                    let id_for_class = s.id;
                    let id_for_click = s.id;
                    let is_sel = move || {
                        selected.get().as_deref() == Some(id_for_class)
                    };
                    let glyph = s.short.to_owned();
                    let toggle = move |_| {
                        selected.update(|cur| {
                            if cur.as_deref() == Some(id_for_click) {
                                *cur = None;
                            } else {
                                *cur = Some(id_for_click.to_owned());
                            }
                        });
                    };
                    view! {
                        <g class="qs-node-group" on:click=toggle>
                            <circle
                                cx=s.x
                                cy=s.y
                                r=move || if is_sel() { 14.0 } else { 12.0 }
                                class=move || if is_sel() { "qs-node qs-node-sel" } else { "qs-node" }
                            />
                            <text x=s.x y=s.y class="qs-node-label">{glyph}</text>
                        </g>
                    }
                }
            />
        </svg>
    }
}

#[component]
fn Region(
    #[expect(unused_variables, reason = "reserved for per-kind styling")] kind: StationKind,
    label: &'static str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> impl IntoView {
    view! {
        <g>
            <rect class="qs-region" x=x y=y width=w height=h rx=6/>
            <text class="qs-region-label" x={x + 6.0} y={y - 4.0}>{label}</text>
        </g>
    }
}
