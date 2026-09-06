//! Developer conveniences: `WT_SCREENSHOT=<png path>` saves a screenshot after 2 s and exits.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

pub fn screenshot_and_exit(
    mut commands: Commands,
    time: Res<Time>,
    mut requested: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(path) = std::env::var("WT_SCREENSHOT") else {
        return;
    };
    if !*requested && time.elapsed_secs() > 2.0 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        *requested = true;
    } else if *requested && time.elapsed_secs() > 3.0 {
        exit.write(AppExit::Success);
    }
}
