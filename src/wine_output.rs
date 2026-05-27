/// Filter Wine diagnostic noise from subprocess stdout/stderr.

pub fn filter_wine_noise(text: &str) -> String {
    text.lines()
        .filter(|line| !is_wine_noise(line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn is_wine_noise(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    // MoltenVK / Vulkan init spam
    if trimmed.starts_with("[mvk-") {
        return true;
    }

    if is_vulkan_or_gpu_dump(trimmed) {
        return true;
    }

    // Wine debug channel lines: "0024:fixme:...", "0024:err:toolbar:...", etc.
    if trimmed.contains(":fixme:")
        || trimmed.contains(":err:")
        || trimmed.contains(":warn:")
    {
        return true;
    }

    false
}

fn is_vulkan_or_gpu_dump(line: &str) -> bool {
    if line.starts_with("VK_") {
        return true;
    }

    if line.contains("Vulkan extension") {
        return true;
    }

    if line.starts_with("The following") && line.contains("Vulkan") {
        return true;
    }

    if line.starts_with("vendorID:")
        || line.starts_with("deviceID:")
        || line.starts_with("pipelineCacheUUID:")
        || line.contains("GPU memory")
    {
        return true;
    }

    if line.starts_with("model:") && line.contains("Graphics") {
        return true;
    }

    if line == "type: Integrated" || line == "type: Discrete" {
        return true;
    }

    if line.contains("Metal Versions")
        || line.starts_with("Metal Shading Language")
        || line.starts_with("GPU Family")
        || line.starts_with("macOS GPU Family")
        || line.starts_with("supports the following Metal")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_fixme() {
        let input = "01d4:fixme:thread:get_thread_times not implemented\nReal output\n";
        assert_eq!(filter_wine_noise(input), "Real output");
    }

    #[test]
    fn removes_toolbar_err() {
        let input = "0024:err:toolbar:ToolbarWindowProc unknown msg 0465, wp 0, lp 21b2d0\n";
        assert_eq!(filter_wine_noise(input), "");
    }

    #[test]
    fn removes_moltenvk() {
        let input = "[mvk-info] MoltenVK version 1.2.7\nActual line\n";
        assert_eq!(filter_wine_noise(input), "Actual line");
    }

    #[test]
    fn keeps_real_output() {
        let input = "strategy.mq5 : 0 error(s), 0 warning(s)\n";
        assert_eq!(filter_wine_noise(input), "strategy.mq5 : 0 error(s), 0 warning(s)");
    }

    #[test]
    fn removes_vulkan_extension_listing() {
        let input = "The following 108 Vulkan extensions are supported:\n\
                     VK_KHR_16bit_storage v1\n\
                     model: Intel(R) Iris(TM) Plus Graphics\n\
                     Real output\n";
        assert_eq!(filter_wine_noise(input), "Real output");
    }

    #[test]
    fn removes_metal_gpu_family_lines() {
        let input = "Metal Shading Language 3.1\nGPU Family Mac 2\nDone.\n";
        assert_eq!(filter_wine_noise(input), "Done.");
    }
}
