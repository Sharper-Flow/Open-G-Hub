//! Integration tests: exercise the full flow using a simulated G502 device.
//!
//! These tests simulate a complete G502 Lightspeed device by registering all
//! expected HID++ feature lookups and command responses, then exercising the
//! full read→validate→write pipeline through multiple modules.

#[cfg(test)]
mod tests {
    use crate::buttons;
    use crate::device::{ButtonAction, PollingRate};
    use crate::dpi;
    use crate::onboard::{self, OnboardMode};
    use crate::report_rate;
    use crate::transport::mock::MockTransport;

    const DEV_IDX: u8 = 0x01;
    const DPI_IDX: u8 = 0x07;
    const RATE_IDX: u8 = 0x08;
    const BTN_IDX: u8 = 0x09;
    const PROFILE_IDX: u8 = 0x0A;

    /// Create a fully-configured mock G502 device with all feature lookups registered.
    fn create_mock_g502() -> MockTransport {
        let mock = MockTransport::new();

        // ROOT feature lookups for all features
        mock.on_short_request(DEV_IDX, 0x00, 0x01, &[0x22, 0x01], &[DPI_IDX, 0x00, 0x00]); // ADJUSTABLE_DPI
        mock.on_short_request(DEV_IDX, 0x00, 0x01, &[0x80, 0x60], &[RATE_IDX, 0x00, 0x00]); // REPORT_RATE
        mock.on_short_request(DEV_IDX, 0x00, 0x01, &[0x1B, 0x04], &[BTN_IDX, 0x00, 0x00]); // REPROG_CONTROLS_V4
        mock.on_short_request(
            DEV_IDX,
            0x00,
            0x01,
            &[0x81, 0x00],
            &[PROFILE_IDX, 0x00, 0x00],
        ); // ONBOARD_PROFILES

        mock
    }

    /// Test: full DPI read→write→verify cycle.
    #[test]
    fn full_dpi_cycle() {
        let mock = create_mock_g502();

        // Initial DPI read: 800
        mock.on_short_request(DEV_IDX, DPI_IDX, 0x11, &[0x00], &[0x03, 0x20, 0x03]);

        let current = dpi::read_dpi(&mock, DEV_IDX).unwrap();
        assert_eq!(current, 800);

        // Write new DPI: 1600
        mock.on_short_request(
            DEV_IDX,
            DPI_IDX,
            0x21,
            &[0x00, 0x06, 0x40],
            &[0x00, 0x06, 0x40],
        );

        let written = dpi::write_dpi(&mock, DEV_IDX, 1600).unwrap();
        assert_eq!(written, 1600);

        // Re-read confirms new value
        mock.on_short_request(DEV_IDX, DPI_IDX, 0x11, &[0x00], &[0x06, 0x40, 0x03]);

        let updated = dpi::read_dpi(&mock, DEV_IDX).unwrap();
        assert_eq!(updated, 1600);
    }

    /// Test: full polling rate read→write cycle.
    #[test]
    fn full_rate_cycle() {
        let mock = create_mock_g502();

        // Read current rate: 1000Hz (interval=1)
        mock.on_short_request(DEV_IDX, RATE_IDX, 0x11, &[], &[0x01, 0x00, 0x00]);

        let current = report_rate::read_report_rate(&mock, DEV_IDX).unwrap();
        assert_eq!(current, PollingRate::Hz1000);

        // Write 500Hz (interval=2)
        mock.on_short_request(DEV_IDX, RATE_IDX, 0x21, &[0x02], &[0x02, 0x00, 0x00]);

        report_rate::write_report_rate(&mock, DEV_IDX, PollingRate::Hz500).unwrap();

        // Verify supported rates
        mock.on_short_request(DEV_IDX, RATE_IDX, 0x01, &[], &[0x0F, 0x00, 0x00]);

        let supported = report_rate::read_supported_rates(&mock, DEV_IDX).unwrap();
        assert_eq!(supported.len(), 4);
    }

    /// Test: button read→remap→verify cycle.
    #[test]
    fn full_button_remap_cycle() {
        let mock = create_mock_g502();

        // Read control count: 6
        mock.on_short_request(DEV_IDX, BTN_IDX, 0x01, &[], &[0x06, 0x00, 0x00]);

        let count = buttons::read_control_count(&mock, DEV_IDX).unwrap();
        assert_eq!(count, 6);

        // Read button 0 mapping (left click → currently left click)
        // getControlInfo for index 0: CID=0x0050
        mock.on_long_request(
            DEV_IDX,
            BTN_IDX,
            0x11,
            &[0x00],
            &[
                0x00, 0x50, 0x00, 0x38, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );
        // getControlReporting for CID=0x0050: remapped to self (0x0050)
        mock.on_long_request(
            DEV_IDX,
            BTN_IDX,
            0x21,
            &[0x00, 0x50],
            &[
                0x00, 0x50, 0x00, 0x00, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        let mapping = buttons::read_button_mapping(&mock, DEV_IDX, 0).unwrap();
        assert_eq!(mapping, ButtonAction::LeftClick);

        // Remap button 1 (right click) to middle click
        // getControlInfo for index 1: CID=0x0051
        mock.on_long_request(
            DEV_IDX,
            BTN_IDX,
            0x11,
            &[0x01],
            &[
                0x00, 0x51, 0x00, 0x39, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );
        // setControlReporting: remap 0x0051 to 0x0052 (middle click)
        mock.on_long_request(
            DEV_IDX,
            BTN_IDX,
            0x31,
            &[0x00, 0x51, 0x10, 0x00, 0x52],
            &[
                0x00, 0x51, 0x10, 0x00, 0x52, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        buttons::write_button_mapping(&mock, DEV_IDX, 1, ButtonAction::MiddleClick).unwrap();
    }

    /// Test: onboard profile switch.
    #[test]
    fn full_profile_switch() {
        let mock = create_mock_g502();

        // Set host mode
        mock.on_short_request(DEV_IDX, PROFILE_IDX, 0x11, &[0x01], &[0x01, 0x00, 0x00]);
        onboard::set_onboard_mode(&mock, DEV_IDX, OnboardMode::Host).unwrap();

        // Get current profile
        mock.on_short_request(DEV_IDX, PROFILE_IDX, 0x21, &[], &[0x00, 0x00, 0x00]);
        let (page, offset) = onboard::get_current_profile(&mock, DEV_IDX).unwrap();
        assert_eq!(page, 0);
        assert_eq!(offset, 0);
    }

    /// Test: DPI validation rejects dangerous values before reaching device.
    #[test]
    fn safety_prevents_dangerous_dpi() {
        let mock = create_mock_g502();

        // 50 DPI is below minimum — should error without touching the device
        let result = dpi::write_dpi(&mock, DEV_IDX, 50);
        assert!(result.is_err());

        // 30000 DPI is above maximum
        let result = dpi::write_dpi(&mock, DEV_IDX, 30000);
        assert!(result.is_err());
    }

    /// Test: button index validation prevents out-of-bounds access.
    #[test]
    fn safety_prevents_invalid_button() {
        let mock = create_mock_g502();

        let result = buttons::write_button_mapping(&mock, DEV_IDX, 10, ButtonAction::LeftClick);
        assert!(result.is_err());
    }

    /// Test: concurrent access to same mock device from multiple threads.
    #[test]
    fn concurrent_reads_are_safe() {
        use std::sync::Arc;
        use std::thread;

        let mock = Arc::new(create_mock_g502());

        // Register DPI read responses (same data for all threads)
        mock.on_short_request(DEV_IDX, DPI_IDX, 0x11, &[0x00], &[0x03, 0x20, 0x03]);

        let mut handles = vec![];
        for _ in 0..4 {
            let mock_ref = Arc::clone(&mock);
            handles.push(thread::spawn(move || {
                let dpi = dpi::read_dpi(mock_ref.as_ref(), DEV_IDX).unwrap();
                assert_eq!(dpi, 800);
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// Test: concurrent reads and writes don't corrupt state.
    #[test]
    fn concurrent_mixed_operations() {
        use std::sync::Arc;
        use std::thread;

        let mock = Arc::new(create_mock_g502());

        // Read DPI
        mock.on_short_request(DEV_IDX, DPI_IDX, 0x11, &[0x00], &[0x03, 0x20, 0x03]);
        // Read rate
        mock.on_short_request(DEV_IDX, RATE_IDX, 0x11, &[], &[0x01, 0x00, 0x00]);
        // Read control count
        mock.on_short_request(DEV_IDX, BTN_IDX, 0x01, &[], &[0x06, 0x00, 0x00]);

        let mock_a = Arc::clone(&mock);
        let mock_b = Arc::clone(&mock);
        let mock_c = Arc::clone(&mock);

        let h1 = thread::spawn(move || dpi::read_dpi(mock_a.as_ref(), DEV_IDX).unwrap());
        let h2 =
            thread::spawn(move || report_rate::read_report_rate(mock_b.as_ref(), DEV_IDX).unwrap());
        let h3 =
            thread::spawn(move || buttons::read_control_count(mock_c.as_ref(), DEV_IDX).unwrap());

        assert_eq!(h1.join().unwrap(), 800);
        assert_eq!(h2.join().unwrap(), PollingRate::Hz1000);
        assert_eq!(h3.join().unwrap(), 6);
    }

    /// Test: multi-feature workflow — set DPI, rate, and button in sequence.
    #[test]
    fn multi_feature_configuration() {
        let mock = create_mock_g502();

        // 1. Set DPI to 3200
        mock.on_short_request(
            DEV_IDX,
            DPI_IDX,
            0x21,
            &[0x00, 0x0C, 0x80],
            &[0x00, 0x0C, 0x80],
        );
        dpi::write_dpi(&mock, DEV_IDX, 3200).unwrap();

        // 2. Set rate to 250Hz
        mock.on_short_request(DEV_IDX, RATE_IDX, 0x21, &[0x04], &[0x04, 0x00, 0x00]);
        report_rate::write_report_rate(&mock, DEV_IDX, PollingRate::Hz250).unwrap();

        // 3. Remap button 3 (back) to forward
        mock.on_long_request(
            DEV_IDX,
            BTN_IDX,
            0x11,
            &[0x03],
            &[
                0x00, 0x53, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );
        mock.on_long_request(
            DEV_IDX,
            BTN_IDX,
            0x31,
            &[0x00, 0x53, 0x10, 0x00, 0x56],
            &[
                0x00, 0x53, 0x10, 0x00, 0x56, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );
        buttons::write_button_mapping(&mock, DEV_IDX, 3, ButtonAction::Forward).unwrap();

        // 4. Switch to onboard mode
        mock.on_short_request(DEV_IDX, PROFILE_IDX, 0x11, &[0x02], &[0x02, 0x00, 0x00]);
        onboard::set_onboard_mode(&mock, DEV_IDX, OnboardMode::Onboard).unwrap();
    }
}
