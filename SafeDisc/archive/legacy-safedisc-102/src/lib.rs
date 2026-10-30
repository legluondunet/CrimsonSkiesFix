// Crimson Skies patcher research modules.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

// Isolate SafeDisc protocol research from the production resolution patcher.
// Nothing in this module installs hooks, kernel drivers or modifies game files.
#[cfg(feature = "research")]
pub mod secdrv_protocol;
