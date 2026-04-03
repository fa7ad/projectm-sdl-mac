use crate::app::App;

impl App {
    pub fn update_projectm_window_size(&mut self) {
        let (width, height) = self.window.size_in_pixels();
        self.pm.set_window_size(width as usize, height as usize);
    }
}
