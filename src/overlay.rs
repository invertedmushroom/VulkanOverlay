// Manages overlay content and radial menu rendering

pub struct OverlayContent {
    pub visible: bool,
    pub selected_segment: Option<i32>, // Track the selected segment of the radial menu
    // Add other fields as needed
}

impl OverlayContent {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_segment: None,
            // Initialize other fields
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }
}