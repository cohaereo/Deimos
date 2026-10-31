use std::time::Instant;

fn main() {
    let file = std::env::args().nth(1).expect("Expected input file path");
    let dxil_data = std::fs::read(&file).expect("Failed to read input file");

    let start = Instant::now();
    let (spirv_data, slots) = dxil2spirv::translate(&dxil_data).expect("Translation failed");
    let duration = start.elapsed();

    println!("Translation took: {:?}", duration);
    println!(
        "Slot usage - CBV: {:b}, SRV: {:b}, Sampler: {:b}, UAV: {:b}",
        slots.cbv, slots.srv, slots.sampler, slots.uav
    );
    let output_path = format!("{}.spv", file);
    std::fs::write(&output_path, bytemuck::cast_slice(&spirv_data))
        .expect("Failed to write output file");

    println!("Successfully translated to {}", output_path);
}
