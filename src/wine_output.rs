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

    // Wine debug channel lines: "0024:fixme:...", "0024:err:toolbar:...", etc.
    if trimmed.contains(":fixme:")
        || trimmed.contains(":err:")
        || trimmed.contains(":warn:")
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
}
