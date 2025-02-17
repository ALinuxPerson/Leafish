// Copyright 2016 Matthew Collins
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod server_list;
pub use self::server_list::*;
mod login;
pub use self::login::*;

pub mod connecting;
pub mod delete_server;
pub mod edit_server;

pub mod background;
pub mod chat;
pub mod launcher;
pub mod respawn;
pub mod settings_menu;

pub use self::settings_menu::{AudioSettingsMenu, SettingsMenu, VideoSettingsMenu};

use crate::render::Renderer;
use crate::ui;
use crate::ui::Container;
use crate::{render, Game};
use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, Position};
use winit::event::VirtualKeyCode;
use winit::window::Window;

pub trait Screen {
    // Called once
    fn init(&mut self, _renderer: &mut render::Renderer, _ui_container: &mut ui::Container) {}
    fn deinit(&mut self, _renderer: &mut render::Renderer, _ui_container: &mut ui::Container) {}

    // May be called multiple times
    fn on_active(&mut self, renderer: &mut render::Renderer, ui_container: &mut ui::Container);
    fn on_deactive(&mut self, renderer: &mut render::Renderer, ui_container: &mut ui::Container);

    // Called every frame the screen is active
    fn tick(
        &mut self,
        delta: f64,
        renderer: &mut render::Renderer,
        ui_container: &mut ui::Container,
    ) -> Option<Box<dyn Screen>>;

    // Events
    fn on_scroll(&mut self, _x: f64, _y: f64) {}

    fn on_resize(&mut self, _renderer: &mut Renderer, _ui_container: &mut Container) {} // TODO: make non-optional!

    fn on_key_press(&mut self, key: VirtualKeyCode, down: bool, game: &mut Game) -> bool {
        if key == VirtualKeyCode::Escape && !down && self.is_closable() {
            game.screen_sys.pop_screen();
            return true;
        }
        false
    }

    fn on_char_receive(&mut self, _received: char, _game: &mut Game) {}

    fn is_closable(&self) -> bool {
        false
    }

    fn is_in_game(&self) -> bool {
        false
    }

    fn is_tick_always(&self) -> bool {
        false
    }

    fn clone_screen(&self) -> Box<dyn Screen>;
}

impl Clone for Box<dyn Screen> {
    fn clone(&self) -> Box<dyn Screen> {
        self.clone_screen()
    }
}

#[derive(Clone)]
struct ScreenInfo {
    screen: Arc<Mutex<Box<dyn Screen>>>,
    init: bool,
    active: bool,
    last_width: i32,
    last_height: i32,
}

// TODO: Add safety comment!
unsafe impl Send for ScreenSystem {}
unsafe impl Sync for ScreenSystem {}

#[derive(Default)]
pub struct ScreenSystem {
    screens: Arc<RwLock<Vec<ScreenInfo>>>,
    pre_computed_screens: Arc<RwLock<Vec<ScreenInfo>>>,
    remove_queue: Arc<RwLock<Vec<ScreenInfo>>>,
    lowest_offset: AtomicIsize,
}

impl ScreenSystem {
    pub fn new() -> ScreenSystem {
        Default::default()
    }

    pub fn add_screen(&self, screen: Box<dyn Screen>) {
        let new_offset = self.pre_computed_screens.clone().read().len() as isize;
        self.pre_computed_screens.clone().write().push(ScreenInfo {
            screen: Arc::new(Mutex::new(screen)),
            init: false,
            active: false,
            last_width: -1,
            last_height: -1,
        });
        let curr_offset = self.lowest_offset.load(Ordering::Acquire);
        if curr_offset == -1 {
            self.lowest_offset.store(new_offset, Ordering::Release);
        }
    }

    pub fn close_closable_screens(&self) {
        loop {
            if !self.is_current_closable() {
                break;
            }
            self.pop_screen();
        }
    }

    pub fn pop_screen(&self) {
        if self.pre_computed_screens.clone().read().last().is_some() {
            // TODO: Improve thread safety (becuz of possible race conditions (which are VERY UNLIKELY to happen - and only if screens get added and removed very fast (in one tick)))
            self.remove_queue
                .clone()
                .write()
                .push(self.pre_computed_screens.clone().write().pop().unwrap());
            let curr_offset = self.lowest_offset.load(Ordering::Acquire);
            let new_offset = self.pre_computed_screens.clone().read().len() as isize;
            if curr_offset == -1 || new_offset < curr_offset {
                self.lowest_offset.store(new_offset, Ordering::Release);
            }
        }
    }

    pub fn replace_screen(&self, screen: Box<dyn Screen>) {
        self.pop_screen();
        self.add_screen(screen);
    }

    pub fn is_current_closable(&self) -> bool {
        if let Some(last) = self.pre_computed_screens.clone().read().last() {
            last.screen.clone().lock().is_closable()
        } else {
            false
        }
    }

    pub fn is_current_ingame(&self) -> bool {
        if let Some(last) = self.pre_computed_screens.clone().read().last() {
            last.screen.clone().lock().is_in_game()
        } else {
            false
        }
    }

    pub fn is_any_ingame(&self) -> bool {
        for screen in self.pre_computed_screens.clone().read().iter().rev() {
            if !screen.screen.clone().is_locked() && screen.screen.clone().lock().is_in_game() {
                return true;
            }
        }
        false
    }

    pub fn receive_char(&self, received: char, game: &mut Game) {
        if self.screens.clone().read().last().is_some() {
            self.screens
                .clone()
                .read()
                .last()
                .as_ref()
                .unwrap()
                .screen
                .clone()
                .lock()
                .on_char_receive(received, game);
        }
    }

    pub fn press_key(&self, key: VirtualKeyCode, down: bool, game: &mut Game) {
        if self.screens.clone().read().last().is_some() {
            self.screens
                .clone()
                .read()
                .last()
                .as_ref()
                .unwrap()
                .screen
                .clone()
                .lock()
                .on_key_press(key, down, game);
        }
    }

    #[allow(unused_must_use)]
    pub fn tick(
        &self,
        delta: f64,
        renderer: Arc<RwLock<render::Renderer>>,
        ui_container: &mut ui::Container,
        window: &Window,
    ) -> bool {
        let renderer = &mut renderer.write();
        for screen in self.remove_queue.clone().write().drain(..) {
            if screen.active {
                screen
                    .screen
                    .clone()
                    .lock()
                    .on_deactive(renderer, ui_container);
            }
            if screen.init {
                screen.screen.clone().lock().deinit(renderer, ui_container);
            }
        }
        let lowest = self.lowest_offset.load(Ordering::Acquire);
        if lowest != -1 {
            let screens_len = self.screens.read().len();
            let was_closable = if screens_len > 0 {
                self.screens
                    .read()
                    .last()
                    .as_ref()
                    .unwrap()
                    .screen
                    .lock()
                    .is_closable()
            } else {
                false
            };
            if lowest <= screens_len as isize {
                for _ in 0..(screens_len as isize - lowest) {
                    self.screens.clone().write().pop();
                }
            }
            for screen in self
                .pre_computed_screens
                .read()
                .iter()
                .skip(lowest as usize)
            {
                let idx = self.screens.read().len() - 1;
                self.screens.write().push(screen.clone());
                let mut screens = self.screens.write();
                let last = screens.get_mut(idx);
                if let Some(last) = last {
                    if last.active {
                        last.active = false;
                        last.screen
                            .clone()
                            .lock()
                            .on_deactive(renderer, ui_container);
                    }
                }
                let mut current = screens.last_mut().unwrap();
                current.init = true;
                current.screen.clone().lock().init(renderer, ui_container);
                current.active = true;
                current
                    .screen
                    .clone()
                    .lock()
                    .on_active(renderer, ui_container);
            }
            self.lowest_offset.store(-1, Ordering::Release);
            if !was_closable {
                window.set_cursor_position(Position::Physical(PhysicalPosition::new(
                    (renderer.safe_width / 2) as i32,
                    (renderer.safe_height / 2) as i32,
                )));
            }
        }

        if self.screens.clone().read().is_empty() {
            return true;
        }
        // Update state for screens
        let len = self.screens.clone().read().len();
        let swap = {
            let tmp = self.screens.clone();
            let mut tmp = tmp.write();
            let current = tmp.last_mut().unwrap();
            if !current.active {
                current.active = true;
                current
                    .screen
                    .clone()
                    .lock()
                    .on_active(renderer, ui_container);
            }
            if current.last_width != renderer.safe_width as i32
                || current.last_height != renderer.safe_height as i32
            {
                if current.last_width != -1 && current.last_height != -1 {
                    for screen in tmp.iter_mut().enumerate() {
                        if screen.1.screen.clone().lock().is_tick_always() || screen.0 == len - 1 {
                            screen
                                .1
                                .screen
                                .clone()
                                .lock()
                                .on_resize(renderer, ui_container);
                            screen.1.last_width = renderer.safe_width as i32;
                            screen.1.last_height = renderer.safe_height as i32;
                        }
                    }
                } else {
                    current.last_width = renderer.safe_width as i32;
                    current.last_height = renderer.safe_height as i32;
                }
            }
            let mut result = None;
            for screen in tmp.iter_mut().enumerate() {
                if screen.1.screen.clone().lock().is_tick_always() && screen.0 != len - 1 {
                    screen
                        .1
                        .screen
                        .clone()
                        .lock()
                        .tick(delta, renderer, ui_container);
                } else if screen.0 == len - 1 {
                    result = screen
                        .1
                        .screen
                        .clone()
                        .lock()
                        .tick(delta, renderer, ui_container);
                }
            }
            result
        };
        // Handle current
        if let Some(swap) = swap {
            self.replace_screen(swap);
        }
        let len = self.screens.clone().read().len();
        return len == 0
            || !self.screens.clone().read()[len - 1]
                .screen
                .clone()
                .lock()
                .is_in_game();
    }

    pub fn on_scroll(&self, x: f64, y: f64) {
        if self.screens.clone().read().is_empty() {
            return;
        }
        self.screens
            .clone()
            .read()
            .last()
            .unwrap()
            .screen
            .clone()
            .lock()
            .on_scroll(x, y);
    }
}
