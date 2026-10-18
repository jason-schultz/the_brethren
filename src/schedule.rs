//! Shared system sets. Every cross-domain ordering constraint is declared here.

use bevy::prelude::*;

/// Simulation phases, run in `FixedUpdate` in this order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimSet {
    /// Turn standing orders into helm commands.
    Navigation,
    /// Battle: tactics take the helm, and broadsides fire.
    Combat,
    /// Integrate ship motion.
    Physics,
    /// React to what happened this step (arrivals, docking, eating).
    Resolve,
    /// Markets, contracts, and deliveries, once ships have docked.
    Economy,
}

/// Per-frame phases, run in `Update` in this order. These keep running while paused.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameSet {
    /// Turn key presses into controls.
    Controls,
    /// Read controls and the mouse, and emit order messages.
    Input,
    /// Record orders as standing order state on entities.
    Orders,
}

/// Configures set ordering. Added once by the top-level simulation plugin.
pub(crate) fn configure_sets(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (
            SimSet::Navigation,
            SimSet::Combat,
            SimSet::Physics,
            SimSet::Resolve,
            SimSet::Economy,
        )
            .chain(),
    )
    .configure_sets(
        Update,
        (FrameSet::Controls, FrameSet::Input, FrameSet::Orders).chain(),
    );
}
