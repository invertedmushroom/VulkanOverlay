use ash::{vk, Entry, Instance, Device};
use ash::extensions::khr::{Surface, Win32Surface, Swapchain};
use winapi::shared::windef::HWND;
use crate::overlay::OverlayContent;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::ptr;
use std::os::raw::c_void;
use std::time::Instant;
use winapi::um::winuser::GetCursorPos;
use winapi::shared::windef::{POINT, RECT};
use winapi::um::winuser::GetWindowRect;

/// Represents the data passed to the shader via uniform buffer.
#[repr(C, align(16))]
struct UniformBufferObject {
    radius: f32,            // Offset 0
    inner_radius: f32,      // Offset 4
    segments: i32,         
    time: f32,                     
    mouse_pos: [f32; 2],
    segment_gap: f32,
    item_selected: i32,
    _padding0: [f32; 1],
    _padding1: [f32; 4],    
}

/// Renderer struct encapsulates Vulkan objects and handles rendering logic.
pub struct Renderer {
    entry: Entry,
    instance: Instance,
    surface_loader: Surface,
    win32_surface_loader: Win32Surface,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Device,
    graphics_queue: vk::Queue,
    swapchain_loader: Swapchain,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
    max_frames_in_flight: usize,
    swapchain_image_count: usize,
    uniform_buffers: Vec<vk::Buffer>,
    uniform_buffers_memory: Vec<vk::DeviceMemory>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: Vec<vk::DescriptorSet>,
    start_time: Instant,
}

impl Renderer {
    fn update_uniform_buffer(
        &self,
        current_image: usize,
        ubo: &UniformBufferObject,
    ) -> Result<(), String> {
        let buffer_size = std::mem::size_of::<UniformBufferObject>();
        let data_ptr = unsafe {
            self.device.map_memory(
                self.uniform_buffers_memory[current_image],
                0,
                buffer_size as vk::DeviceSize,
                vk::MemoryMapFlags::empty(),
            ).map_err(|e| format!("Failed to map uniform buffer memory: {:?}", e))?
        } as *mut UniformBufferObject;
    
        unsafe {
            data_ptr.copy_from_nonoverlapping(ubo, 1);
            self.device.unmap_memory(self.uniform_buffers_memory[current_image]);
        }
    
        Ok(())
    }
    /// Initializes Vulkan, creates instance, selects physical device, creates logical device, and sets up swapchain.
    pub fn new(hwnd: HWND) -> Result<Self, String> {
        // Initialize Vulkan entry
        let entry = unsafe { Entry::load().map_err(|_| "Failed to load Vulkan entry".to_string())? };

        // Enable validation layers in debug mode
        let enable_validation_layers = cfg!(debug_assertions);
        let validation_layers = [CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
        let layer_names: Vec<*const i8> = validation_layers.iter().map(|layer| layer.as_ptr()).collect();

        // Create Vulkan instance
        let app_name = CString::new("Vulkan Overlay").unwrap();
        let engine_name = CString::new("No Engine").unwrap();

        let app_info = vk::ApplicationInfo::builder()
            .application_name(&app_name)
            .engine_name(&engine_name)
            .application_version(0)
            .engine_version(0)
            .api_version(vk::API_VERSION_1_0);

        // Required extensions for Windows surface
        let extension_names = vec![
            Surface::name().as_ptr(),
            Win32Surface::name().as_ptr(),
        ];

        let mut instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        if enable_validation_layers {
            instance_create_info = instance_create_info.enabled_layer_names(&layer_names);
        }

        let instance = unsafe {
            entry
                .create_instance(&instance_create_info, None)
                .map_err(|e| format!("Failed to create Vulkan instance: {:?}", e))?
        };

        // Create surface for rendering
        let surface_loader = Surface::new(&entry, &instance);
        let win32_surface_loader = Win32Surface::new(&entry, &instance);

        let hwnd_ptr = hwnd as *mut c_void;
        let hinstance = unsafe { winapi::um::libloaderapi::GetModuleHandleW(ptr::null()) };

        let win32_create_info = vk::Win32SurfaceCreateInfoKHR::builder()
            .hinstance(hinstance as *mut c_void)
            .hwnd(hwnd_ptr);

        let surface = unsafe {
            win32_surface_loader
                .create_win32_surface(&win32_create_info, None)
                .map_err(|e| format!("Failed to create Win32 surface: {:?}", e))?
        };

        // Pick a physical device
        let physical_device = pick_physical_device(&instance, &surface_loader, surface)?;

        // Find queue family index
        let queue_family_index = find_queue_family_index(&instance, physical_device, &surface_loader, surface)?;

        // Create logical device and get graphics queue
        let (device, graphics_queue) = create_logical_device_and_queue(
            &instance,
            physical_device,
            queue_family_index,
            enable_validation_layers,
            &layer_names, // Pass layer_names to maintain their lifetime
        )?;

        // Create swapchain loader
        let swapchain_loader = Swapchain::new(&instance, &device);

        // Create swapchain
        let (swapchain, swapchain_image_format, swapchain_extent) = create_swapchain(
            &surface_loader,
            &swapchain_loader,
            &device,
            physical_device,
            surface,
            queue_family_index,
        )?;

        // Retrieve swapchain images
        let swapchain_images = unsafe {
            swapchain_loader
                .get_swapchain_images(swapchain)
                .map_err(|e| format!("Failed to get swapchain images: {:?}", e))?
        };

        let swapchain_image_count = swapchain_images.len();
        let max_frames_in_flight = 2; // Double buffering

        // Create image views for swapchain images
        let swapchain_image_views = create_image_views(&device, &swapchain_images, swapchain_image_format)?;

        // Create render pass
        let render_pass = create_render_pass(&device, swapchain_image_format)?;

        // Create framebuffers
        let framebuffers = create_framebuffers(
            &device,
            render_pass,
            &swapchain_image_views,
            swapchain_extent,
        )?;
        // Create descriptor set layout
        let descriptor_set_layout = create_descriptor_set_layout(&device)?;

        // Create uniform buffers
        let (uniform_buffers, uniform_buffers_memory) = create_uniform_buffers(
            &instance,
            &device,
            physical_device,
            swapchain_images.len(),
            
        )?;

        // Create descriptor pool
        let descriptor_pool = create_descriptor_pool(&device, swapchain_images.len())?;

        // Create descriptor sets
        let descriptor_sets = create_descriptor_sets(
            &device,
            descriptor_pool,
            descriptor_set_layout,
            &uniform_buffers,
        )?;

        // Initialize start time
        let start_time = Instant::now();
        
        // Create graphics pipeline
        let (pipeline_layout, graphics_pipeline) = create_graphics_pipeline(&device, render_pass, swapchain_extent, descriptor_set_layout)?;

        // Create command pool
        let command_pool = create_command_pool(&device, queue_family_index)?;

        // Allocate command buffers
        let command_buffers = allocate_command_buffers(&device, command_pool, framebuffers.len())?;

        // Record command buffers
        record_command_buffers(
            &device,
            &command_buffers,
            render_pass,
            &framebuffers,
            graphics_pipeline,
            swapchain_extent,
            pipeline_layout,
            &descriptor_sets,
        )?;

        // Create synchronization objects
        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) = create_sync_objects(&device, max_frames_in_flight)?;

        Ok(Self {
            entry,
            instance,
            surface_loader,
            win32_surface_loader,
            surface,
            physical_device,
            device,
            graphics_queue,
            swapchain_loader,
            swapchain,
            swapchain_images,
            swapchain_image_format,
            swapchain_extent,
            swapchain_image_views,
            render_pass,
            framebuffers,
            pipeline_layout,
            graphics_pipeline,
            command_pool,
            command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            current_frame: 0,
            max_frames_in_flight,
            swapchain_image_count,
            uniform_buffers,
            uniform_buffers_memory,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            start_time,
        })
    }

    /// Renders a frame. This function should be called every frame when the overlay is visible.
    pub fn render(&mut self, _overlay_content: &mut OverlayContent, hwnd: HWND) -> Result<(), String> {
        // Wait for the fence of the current frame to be signaled
        unsafe {
            self.device
                .wait_for_fences(&[self.in_flight_fences[self.current_frame]], true, std::u64::MAX)
                .map_err(|e| format!("Failed to wait for fence: {:?}", e))?;
            self.device
                .reset_fences(&[self.in_flight_fences[self.current_frame]])
                .map_err(|e| format!("Failed to reset fence: {:?}", e))?;
        }

        // Acquire an image from the swapchain
        let (image_index, _) = unsafe {
            self.swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    std::u64::MAX,
                    self.image_available_semaphores[self.current_frame],
                    vk::Fence::null(),
                )
                .map_err(|e| format!("Failed to acquire next image: {:?}", e))?
        };

        // Get mouse position
        let mut point: POINT = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point);
        }

        // Get window position
        let mut window_rect: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe {
            GetWindowRect(hwnd, &mut window_rect);
        }

        // Calculate mouse position relative to the window
        let mouse_x = point.x - window_rect.left;
        let mouse_y = point.y - window_rect.top;

        // Window dimensions
        //let window_width_debug = self.swapchain_extent.width as f32; 
        //let window_height_debug = self.swapchain_extent.height as f32;
        
        let window_width = window_rect.right - window_rect.left;
        let window_height = window_rect.bottom - window_rect.top;
        // Normalize mouse position to range [-1, 1]
        // X goes from -1 (left) to 1 (right)
        // Y goes from -1 (bottom) to 1 (top)
        let normalized_mouse_x = (mouse_x as f32 / window_width as f32) * 2.0 - 1.0;
        let normalized_mouse_y = 1.0 - (mouse_y as f32 / window_height as f32) * 2.0;  
        //debug
        //println!("Mouse X: {}, Normalized X: {}", mouse_x, normalized_mouse_x);
        //println!("Mouse Y: {}, Normalized Y: {}", mouse_y, normalized_mouse_y);
        //println!("window_width_deb: {}, window_width: {}", window_width_debug, window_width);
        //println!("window_height_deb: {}, window_height: {}", window_height_debug, window_height);

        update_selection(normalized_mouse_x, normalized_mouse_y, _overlay_content);

        // Update the uniform buffer
        let current_time = self.start_time.elapsed().as_secs_f32();
        let ubo = UniformBufferObject {
            radius: 0.25,
            inner_radius: 0.08,
            segments: 6,
            time: current_time,
            mouse_pos: [normalized_mouse_x, normalized_mouse_y],
            segment_gap: 0.1,
            item_selected: _overlay_content.selected_segment.unwrap_or(-1),
            _padding0: [0.0],
            _padding1: [0.0, 0.0, 0.0, 0.0],
        };

        self.update_uniform_buffer(image_index as usize, &ubo)?;

        // Submit the command buffer
        let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished_semaphores[self.current_frame]];
        let command_buffers_to_submit = [self.command_buffers[image_index as usize]];

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers_to_submit)
            .signal_semaphores(&signal_semaphores)
            .build();

        unsafe {
            self.device
                .queue_submit(
                    self.graphics_queue,
                    &[submit_info],
                    self.in_flight_fences[self.current_frame],
                )
                .map_err(|e| format!("Failed to submit queue: {:?}", e))?;
        }

        // Present the image
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices)
            .build();

        unsafe {
            self.swapchain_loader
                .queue_present(self.graphics_queue, &present_info)
                .map_err(|e| format!("Failed to present queue: {:?}", e))?;
        }

        // Advance to the next frame
        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;

        Ok(())
    }

    /// Cleans up Vulkan resources in reverse order of creation.
    pub fn cleanup(&mut self) {
        unsafe {
            // Wait for the device to finish operations
            self.device.device_wait_idle().unwrap();

            // Destroy synchronization objects
            for &semaphore in self.image_available_semaphores.iter() {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &semaphore in self.render_finished_semaphores.iter() {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &fence in self.in_flight_fences.iter() {
                self.device.destroy_fence(fence, None);
            }

            // Destroy command pool and command buffers
            self.device.destroy_command_pool(self.command_pool, None);

            // Destroy graphics pipeline and layout
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);

            // Destroy framebuffers
            for &framebuffer in self.framebuffers.iter() {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            // Destroy render pass
            self.device.destroy_render_pass(self.render_pass, None);

            // Destroy image views
            for &image_view in self.swapchain_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }

            // Destroy uniform buffers
            for &buffer in self.uniform_buffers.iter() {
                self.device.destroy_buffer(buffer, None);
            }
            for &memory in self.uniform_buffers_memory.iter() {
                self.device.free_memory(memory, None);
            }

            // Destroy descriptor pool and set layout
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            // Destroy swapchain
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);

            // Destroy logical device
            self.device.destroy_device(None);

            // Destroy surface using surface_loader
            self.surface_loader.destroy_surface(self.surface, None);

            // Destroy Vulkan instance
            self.instance.destroy_instance(None);
        }
    }
}

fn update_selection(normalized_mouse_x: f32, normalized_mouse_y: f32, _overlay_content: &mut OverlayContent) {

    // Calculate mouse position relative to the center of the menu
    let coord_x = normalized_mouse_x;
    let coord_y = -normalized_mouse_y;
    // Invert Y-axis to match shader

    let dist = (coord_x.powi(2) + coord_y.powi(2)).sqrt();

    // Check if mouse is outside the inner_radius
    let inner_radius = 0.08;
    // Should match the value in the shader or from the uniform
    let outer_radius = 0.25;
    // Should match ubo.radius

    let mut selected_segment = None;

    if dist >= inner_radius { //  && dist <= outer_radius
        // Calculate angle
        let mut angle = coord_y.atan2(coord_x);
        if angle < 0.0 {
            angle += 2.0 * std::f32::consts::PI;
        }

        // Calculate segment index
        let segments = 6; // Should match ubo.segments
        let segment_gap = 0.1; // Should match ubo.segment_gap
        let segment_angle_with_gap = (2.0 * std::f32::consts::PI) / segments as f32;

        let segment_index = (angle / segment_angle_with_gap).floor() as i32;

        selected_segment = Some(segment_index);

        // Print when selection changes
        if _overlay_content.selected_segment != Some(segment_index) {
            println!("Selected Segment: {}", segment_index);
            _overlay_content.selected_segment = Some(segment_index);
        }
    } else {
        // Mouse is inside the inner radius or outside the outer radius
        if _overlay_content.selected_segment.is_some() {
            println!("No Segment Selected");
            _overlay_content.selected_segment = None;
        }
    }
}

/// Picks a suitable physical device that supports graphics and presentation.
fn pick_physical_device(instance: &Instance, surface_loader: &Surface, surface: vk::SurfaceKHR) -> Result<vk::PhysicalDevice, String> {
    let physical_devices = unsafe {
        instance
            .enumerate_physical_devices()
            .map_err(|e| format!("Failed to enumerate physical devices: {:?}", e))?
    };

    for device in physical_devices {
        if is_device_suitable(instance, surface_loader, device, surface)? {
            return Ok(device);
        }
    }

    Err("Failed to find a suitable GPU!".to_string())
}

/// Checks if the physical device is suitable by verifying it supports required features.
fn is_device_suitable(instance: &Instance, surface_loader: &Surface, device: vk::PhysicalDevice, surface: vk::SurfaceKHR) -> Result<bool, String> {
    // Check for graphics and presentation support.
    let queue_families = unsafe { instance.get_physical_device_queue_family_properties(device) };

    let mut has_graphics = false;
    let mut has_present = false;

    for (index, queue_family) in queue_families.iter().enumerate() {
        if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            has_graphics = true;
        }

        let present_support = unsafe {
            surface_loader
                .get_physical_device_surface_support(device, index as u32, surface)
                .map_err(|e| format!("Failed to get device surface support: {:?}", e))?
        };

        if present_support {
            has_present = true;
        }

        if has_graphics && has_present {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Finds a suitable queue family index that supports graphics and presentation.
fn find_queue_family_index(instance: &Instance, physical_device: vk::PhysicalDevice, surface_loader: &Surface, surface: vk::SurfaceKHR) -> Result<u32, String> {
    let queue_families = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    for (index, queue_family) in queue_families.iter().enumerate() {
        if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            let present_support = unsafe {
                surface_loader
                    .get_physical_device_surface_support(physical_device, index as u32, surface)
                    .map_err(|e| format!("Failed to get device surface support: {:?}", e))?
            };

            if present_support {
                return Ok(index as u32);
            }
        }
    }

    Err("Failed to find a suitable queue family.".to_string())
}

/// Creates a logical device and retrieves the graphics queue.
fn create_logical_device_and_queue(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    enable_validation_layers: bool,
    layer_names: &[*const i8],
) -> Result<(Device, vk::Queue), String> {
    let queue_priority = [1.0_f32];

    let queue_create_info = vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priority)
        .build();

    let queue_create_infos = [queue_create_info];

    let device_extension_names = [Swapchain::name().as_ptr()];

    let mut device_create_info_builder = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&device_extension_names);

    if enable_validation_layers {
        device_create_info_builder = device_create_info_builder.enabled_layer_names(layer_names);
    }

    let device_create_info = device_create_info_builder.build();

    let device = unsafe {
        instance
            .create_device(physical_device, &device_create_info, None)
            .map_err(|e| format!("Failed to create logical device: {:?}", e))?
    };

    let graphics_queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    Ok((device, graphics_queue))
}

/// Creates the swapchain based on surface capabilities and formats.
fn create_swapchain(
    surface_loader: &Surface,
    swapchain_loader: &Swapchain,
    _device: &Device,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    _queue_family_index: u32,
) -> Result<(vk::SwapchainKHR, vk::Format, vk::Extent2D), String> {
    // Query surface capabilities and formats
    let surface_capabilities = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(physical_device, surface)
            .map_err(|e| format!("Failed to get surface capabilities: {:?}", e))?
    };

    // Choose a suitable surface format
    let surface_formats = unsafe {
        surface_loader
            .get_physical_device_surface_formats(physical_device, surface)
            .map_err(|e| format!("Failed to get surface formats: {:?}", e))?
    };

    let surface_format = if surface_formats.len() == 1 && surface_formats[0].format == vk::Format::UNDEFINED {
        vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        }
    } else {
        surface_formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .cloned()
            .unwrap_or(surface_formats[0])
    };

    // Choose present mode
    let present_modes = unsafe {
        surface_loader
            .get_physical_device_surface_present_modes(physical_device, surface)
            .map_err(|e| format!("Failed to get present modes: {:?}", e))?
    };

    let present_mode = present_modes
        .iter()
        .cloned()
        .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO);

    // Choose swap extent
    let swap_extent = if surface_capabilities.current_extent.width != u32::MAX {
        surface_capabilities.current_extent
    } else {
        vk::Extent2D {
            width: 800,  // Replace with actual window width
            height: 600, // Replace with actual window height
        }
    };

    // Choose number of images
    let mut image_count = surface_capabilities.min_image_count + 1;
    if surface_capabilities.max_image_count > 0 && image_count > surface_capabilities.max_image_count {
        image_count = surface_capabilities.max_image_count;
    }

    let composite_alpha_flags = surface_capabilities.supported_composite_alpha;
    println!("Supported composite alpha flags: {:?}", composite_alpha_flags);

    // Update the composite alpha to support transparency
    let composite_alpha = if composite_alpha_flags.contains(vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED) {
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED
    } else if composite_alpha_flags.contains(vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED) {
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED
    } else if composite_alpha_flags.contains(vk::CompositeAlphaFlagsKHR::INHERIT) {
        vk::CompositeAlphaFlagsKHR::INHERIT
    } else {
        vk::CompositeAlphaFlagsKHR::OPAQUE
    };

    println!("Selected composite alpha: {:?}", composite_alpha);

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(swap_extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(surface_capabilities.current_transform)
        .composite_alpha(composite_alpha)
        .present_mode(present_mode)
        .clipped(true)
        .build();

    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&swapchain_create_info, None)
            .map_err(|e| format!("Failed to create swapchain: {:?}", e))?
    };

    Ok((swapchain, surface_format.format, swap_extent))
}

/// Creates image views for each swapchain image.
fn create_image_views(
    device: &Device,
    swapchain_images: &Vec<vk::Image>,
    swapchain_image_format: vk::Format,
) -> Result<Vec<vk::ImageView>, String> {
    let mut swapchain_image_views = Vec::new();

    for &image in swapchain_images.iter() {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(swapchain_image_format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .build();

        let image_view = unsafe {
            device
                .create_image_view(&create_info, None)
                .map_err(|e| format!("Failed to create image view: {:?}", e))?
        };

        swapchain_image_views.push(image_view);
    }

    Ok(swapchain_image_views)
}

/// Creates a render pass for rendering operations.
fn create_render_pass(device: &Device, swapchain_image_format: vk::Format) -> Result<vk::RenderPass, String> {
    let color_attachment = vk::AttachmentDescription::builder()
        .format(swapchain_image_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .build();

    let color_attachment_ref = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };

    let subpass = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_attachment_ref))
        .build();

    let dependency = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
        .build();

    let render_pass_create_info = vk::RenderPassCreateInfo::builder()
        .attachments(std::slice::from_ref(&color_attachment))
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dependency));

    let render_pass = unsafe {
        device
            .create_render_pass(&render_pass_create_info, None)
            .map_err(|e| format!("Failed to create render pass: {:?}", e))?
    };

    Ok(render_pass)
}

/// Creates framebuffers for each swapchain image view.
fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    swapchain_image_views: &Vec<vk::ImageView>,
    swapchain_extent: vk::Extent2D,
) -> Result<Vec<vk::Framebuffer>, String> {
    let mut framebuffers = Vec::new();

    for &image_view in swapchain_image_views.iter() {
        let attachments = [image_view];

        let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(swapchain_extent.width)
            .height(swapchain_extent.height)
            .layers(1)
            .build();

        let framebuffer = unsafe {
            device
                .create_framebuffer(&framebuffer_create_info, None)
                .map_err(|e| format!("Failed to create framebuffer: {:?}", e))?
        };

        framebuffers.push(framebuffer);
    }

    Ok(framebuffers)
}

/// Creates a graphics pipeline with simple shaders.
fn create_graphics_pipeline(device: &Device, render_pass: vk::RenderPass, swapchain_extent: vk::Extent2D, descriptor_set_layout: vk::DescriptorSetLayout,) -> Result<(vk::PipelineLayout, vk::Pipeline), String> {
    // Load shader modules
    let vert_shader_code = read_spirv_shader("shaders/vert.spv")?;
    let frag_shader_code = read_spirv_shader("shaders/frag.spv")?;

    let vert_shader_module = unsafe {
        device
            .create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&vert_shader_code), None)
            .map_err(|e| format!("Failed to create vertex shader module: {:?}", e))?
    };

    let frag_shader_module = unsafe {
        device
            .create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&frag_shader_code), None)
            .map_err(|e| format!("Failed to create fragment shader module: {:?}", e))?
    };

    // Vertex shader stage
    let shader_entry_name = CString::new("main").unwrap();
    let vert_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(vert_shader_module)
        .name(&shader_entry_name)
        .build();

    // Fragment shader stage
    let frag_shader_stage_info = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(frag_shader_module)
        .name(&shader_entry_name)
        .build();

    let shader_stages = [vert_shader_stage_info, frag_shader_stage_info];

    // Vertex input
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&[])
        .vertex_attribute_descriptions(&[]);

    // Input assembly
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    // Viewport and scissor
    let viewport = vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: swapchain_extent.width as f32,
        height: swapchain_extent.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    };

    let scissor = vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: swapchain_extent,
    };

    // Bind viewports and scissors to variables to extend their lifetimes
    let viewports = [viewport];
    let scissors = [scissor];

    let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
        .viewports(&viewports)
        .scissors(&scissors);

    // Rasterizer
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::BACK)
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(false);

    // Multisampling
    let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
        .sample_shading_enable(false)
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    // Color blending
    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
        .color_write_mask(
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
        )
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .build();
        
        let color_blend_attachments = [color_blend_attachment]; //array reference gets dropped so binding is defined here
        let color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
        .logic_op_enable(false)
        .logic_op(vk::LogicOp::COPY)
        .attachments(&color_blend_attachments)
        .blend_constants([0.0, 0.0, 0.0, 0.0]);

    // Pipeline layout
    let binding = [descriptor_set_layout];
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
    .set_layouts(&binding)
        .push_constant_ranges(&[]);

    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&pipeline_layout_info, None)
            .map_err(|e| format!("Failed to create pipeline layout: {:?}", e))?
    };

    // Graphics pipeline
    let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
        .stages(&shader_stages)
        .vertex_input_state(&vertex_input_info)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisampling)
        .color_blend_state(&color_blending)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0)
        .base_pipeline_handle(vk::Pipeline::null());

    let graphics_pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info.build()], None)
            .map_err(|e| format!("Failed to create graphics pipeline: {:?}", e))?
            .remove(0)
    };

    // Destroy shader modules after pipeline creation
    unsafe {
        device.destroy_shader_module(vert_shader_module, None);
        device.destroy_shader_module(frag_shader_module, None);
    }

    Ok((pipeline_layout, graphics_pipeline))
}

/// Reads a SPIR-V shader file and returns its contents as a Vec<u32>.
fn read_spirv_shader<P: AsRef<Path>>(path: P) -> Result<Vec<u32>, String> {
    let mut file = File::open(path.as_ref()).map_err(|e| format!("Failed to open shader file: {:?}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| format!("Failed to read shader file: {:?}", e))?;

    // Ensure the buffer length is a multiple of 4
    if buffer.len() % 4 != 0 {
        return Err("Shader bytecode is not properly aligned.".to_string());
    }

    // Convert bytes to u32
    let spirv = buffer
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    Ok(spirv)
}

/// Creates a command pool for allocating command buffers.
fn create_command_pool(device: &Device, queue_family_index: u32) -> Result<vk::CommandPool, String> {
    let pool_info = vk::CommandPoolCreateInfo::builder()
        .queue_family_index(queue_family_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

    let command_pool = unsafe {
        device
            .create_command_pool(&pool_info, None)
            .map_err(|e| format!("Failed to create command pool: {:?}", e))?
    };

    Ok(command_pool)
}

/// Allocates command buffers from the command pool.
fn allocate_command_buffers(device: &Device, command_pool: vk::CommandPool, count: usize) -> Result<Vec<vk::CommandBuffer>, String> {
    let allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(count as u32);

    let command_buffers = unsafe {
        device
            .allocate_command_buffers(&allocate_info)
            .map_err(|e| format!("Failed to allocate command buffers: {:?}", e))?
    };

    Ok(command_buffers)
}

/// Records command buffers to draw a simple triangle.
fn record_command_buffers(
    device: &Device,
    command_buffers: &[vk::CommandBuffer],
    render_pass: vk::RenderPass,
    framebuffers: &Vec<vk::Framebuffer>,
    graphics_pipeline: vk::Pipeline,
    swapchain_extent: vk::Extent2D,
    pipeline_layout: vk::PipelineLayout,
    descriptor_sets: &[vk::DescriptorSet],
) -> Result<(), String> {
    for (i, &command_buffer) in command_buffers.iter().enumerate() {
        let begin_info = vk::CommandBufferBeginInfo::builder();

        unsafe {
            device
                .begin_command_buffer(command_buffer, &begin_info)
                .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;
        }

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [1.0, 0.0, 1.0, 1.0], // Fully transparent
            },
        }];

        let render_pass_info = vk::RenderPassBeginInfo::builder()
            .render_pass(render_pass)
            .framebuffer(framebuffers[i])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: swapchain_extent,
            })
            .clear_values(&clear_values);

        unsafe {
            device.cmd_begin_render_pass(command_buffer, &render_pass_info, vk::SubpassContents::INLINE);
            device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, graphics_pipeline);
            
            // Bind the descriptor set
            device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0,
                &[descriptor_sets[i]],
                &[],
            );

            // Update the draw call to draw 4 vertices for the quad
            device.cmd_draw(command_buffer, 6, 1, 0, 0);
            device.cmd_end_render_pass(command_buffer);
            device
                .end_command_buffer(command_buffer)
                .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;
        }
    }

    Ok(())
}

/// Creates synchronization objects: semaphores and fences.
fn create_sync_objects(device: &Device, max_frames_in_flight: usize) -> Result<(Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>), String> {
    let semaphore_info = vk::SemaphoreCreateInfo::builder();
    let fence_info = vk::FenceCreateInfo::builder()
        .flags(vk::FenceCreateFlags::SIGNALED); // Start signaled to avoid waiting on first frame

    let mut image_available_semaphores = Vec::with_capacity(max_frames_in_flight);
    let mut render_finished_semaphores = Vec::with_capacity(max_frames_in_flight);
    let mut in_flight_fences = Vec::with_capacity(max_frames_in_flight);

    for _ in 0..max_frames_in_flight {
        let image_available = unsafe {
            device
                .create_semaphore(&semaphore_info, None)
                .map_err(|e| format!("Failed to create image_available semaphore: {:?}", e))?
        };
        let render_finished = unsafe {
            device
                .create_semaphore(&semaphore_info, None)
                .map_err(|e| format!("Failed to create render_finished semaphore: {:?}", e))?
        };
        let fence = unsafe {
            device
                .create_fence(&fence_info, None)
                .map_err(|e| format!("Failed to create fence: {:?}", e))?
        };

        image_available_semaphores.push(image_available);
        render_finished_semaphores.push(render_finished);
        in_flight_fences.push(fence);
    }

    Ok((image_available_semaphores, render_finished_semaphores, in_flight_fences))
}

/// Creates a descriptor set layout for the uniform buffer.
fn create_descriptor_set_layout(device: &Device) -> Result<vk::DescriptorSetLayout, String> {
    let ubo_layout_binding = vk::DescriptorSetLayoutBinding::builder()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        .build();

    let bindings = [ubo_layout_binding];

    let layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
        .bindings(&bindings);

    let descriptor_set_layout = unsafe {
        device.create_descriptor_set_layout(&layout_info, None)
            .map_err(|e| format!("Failed to create descriptor set layout: {:?}", e))?
    };

    Ok(descriptor_set_layout)
}

/// Creates uniform buffers for each swapchain image.
fn create_uniform_buffers(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    swapchain_image_count: usize,
) -> Result<(Vec<vk::Buffer>, Vec<vk::DeviceMemory>), String> {
    let buffer_size = std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
    let mut uniform_buffers = Vec::with_capacity(swapchain_image_count);
    let mut uniform_buffers_memory = Vec::with_capacity(swapchain_image_count);

    for _ in 0..swapchain_image_count {
        let (buffer, buffer_memory) = create_buffer(
            instance,
            device,
            physical_device,
            buffer_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        uniform_buffers.push(buffer);
        uniform_buffers_memory.push(buffer_memory);
    }

    Ok((uniform_buffers, uniform_buffers_memory))
}

/// Creates a descriptor pool for uniform buffers.
fn create_descriptor_pool(
    device: &Device,
    swapchain_image_count: usize,
) -> Result<vk::DescriptorPool, String> {
    let pool_size = vk::DescriptorPoolSize::builder()
        .ty(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(swapchain_image_count as u32)
        .build();

    let pool_sizes = [pool_size];

    let pool_info = vk::DescriptorPoolCreateInfo::builder()
        .pool_sizes(&pool_sizes)
        .max_sets(swapchain_image_count as u32);

    let descriptor_pool = unsafe {
        device.create_descriptor_pool(&pool_info, None)
            .map_err(|e| format!("Failed to create descriptor pool: {:?}", e))?
    };

    Ok(descriptor_pool)
}

/// Creates descriptor sets for uniform buffers.
fn create_descriptor_sets(
    device: &Device,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    uniform_buffers: &[vk::Buffer],
) -> Result<Vec<vk::DescriptorSet>, String> {
    let layouts = vec![descriptor_set_layout; uniform_buffers.len()];

    let alloc_info = vk::DescriptorSetAllocateInfo::builder()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&layouts);

    let descriptor_sets = unsafe {
        device.allocate_descriptor_sets(&alloc_info)
            .map_err(|e| format!("Failed to allocate descriptor sets: {:?}", e))?
    };

    for (i, &descriptor_set) in descriptor_sets.iter().enumerate() {
        let buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(uniform_buffers[i])
            .offset(0)
            .range(std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize);

        let descriptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(std::slice::from_ref(&buffer_info));

        unsafe {
            device.update_descriptor_sets(&[descriptor_write.build()], &[]);
        }
    }

    Ok(descriptor_sets)
}

/// Helper function to create a buffer.
fn create_buffer(
    instance: &Instance,
    device: &Device,
    physical_device: vk::PhysicalDevice,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), String> {
    let buffer_info = vk::BufferCreateInfo::builder()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe {
        device.create_buffer(&buffer_info, None)
            .map_err(|e| format!("Failed to create buffer: {:?}", e))?
    };

    let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

    let mem_properties = unsafe {
        instance.get_physical_device_memory_properties(physical_device)
    };

    let memory_type = find_memory_type(
        mem_requirements.memory_type_bits,
        properties,
        mem_properties,
    )?;

    let alloc_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(mem_requirements.size)
        .memory_type_index(memory_type);

    let buffer_memory = unsafe {
        device.allocate_memory(&alloc_info, None)
            .map_err(|e| format!("Failed to allocate buffer memory: {:?}", e))?
    };

    unsafe {
        device.bind_buffer_memory(buffer, buffer_memory, 0)
            .map_err(|e| format!("Failed to bind buffer memory: {:?}", e))?;
    }

    Ok((buffer, buffer_memory))
}

/// Helper function to find a suitable memory type.
fn find_memory_type(
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
    mem_properties: vk::PhysicalDeviceMemoryProperties,
) -> Result<u32, String> {
    for i in 0..mem_properties.memory_type_count {
        if (type_filter & (1 << i)) != 0 &&
            mem_properties.memory_types[i as usize].property_flags.contains(properties) {
            return Ok(i);
        }
    }
    Err("Failed to find suitable memory type.".to_string())
}