# Radial Menu Overlay

## Description
AI generated code for windows that displays a fully transparent, click-through radial overlay.

https://github.com/user-attachments/assets/0f695547-a939-4532-99a3-a3129f119056

## Features
- Fully transparent and click-through window overlay.
- Shader based radial menu rendered using Vulkan
- Mouse position is passed to GPU
- Hotkey ALT + R to display

### Compile shaders
glslangValidator -V shaders/vert.vert.glsl -o shaders/vert.spv

compile the one you want to use
glslangValidator -V shaders/indexFromRust.frag.glsl -o shaders/frag.spv


