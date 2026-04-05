//! Runtime system information for the T-Deck.

use esp_hal::Blocking;
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
use esp_hal::analog::adc::AdcCalCurve;
use esp_hal::peripherals::{ADC1, GPIO4};

const BATTERY_DIVIDER_RATIO: u32 = 2;
const BATTERY_EMPTY_MV: u32 = 3300;
const BATTERY_FULL_MV: u32 = 4200;

/// Snapshot of system metrics rendered in the info overlay.
pub struct SystemInfoSnapshot {
    /// Estimated battery charge percentage.
    pub battery_percent: u8,
    /// Estimated battery voltage in millivolts.
    pub battery_mv: u32,
    /// Heap bytes currently used.
    pub ram_used_bytes: usize,
    /// Total configured heap bytes.
    pub ram_total_bytes: usize,
}

/// Reads system metrics from ADC and heap allocator state.
pub struct SystemInfoReader<'d> {
    adc: Adc<'d, ADC1<'d>, Blocking>,
    battery_pin: esp_hal::analog::adc::AdcPin<GPIO4<'d>, ADC1<'d>, AdcCalCurve<ADC1<'d>>>,
}

impl<'d> SystemInfoReader<'d> {
    /// Creates a system info reader for the T-Deck battery ADC input.
    pub fn new(adc1: ADC1<'d>, gpio4: GPIO4<'d>) -> Self {
        let mut adc1_config: AdcConfig<ADC1<'d>> = AdcConfig::new();
        let battery_pin = adc1_config
            .enable_pin_with_cal::<_, AdcCalCurve<ADC1<'d>>>(gpio4, Attenuation::_11dB);
        let adc = Adc::new(adc1, adc1_config);
        Self { adc, battery_pin }
    }

    /// Captures the current battery estimate and heap usage.
    pub fn snapshot(&mut self) -> SystemInfoSnapshot {
        let battery_pin_mv: u32 = u32::from(self.adc.read_blocking(&mut self.battery_pin));
        let battery_mv = battery_pin_mv.saturating_mul(BATTERY_DIVIDER_RATIO);
        let battery_percent = battery_percent_from_mv(battery_mv);

        let heap_stats = esp_alloc::HEAP.stats();
        SystemInfoSnapshot {
            battery_percent,
            battery_mv,
            ram_used_bytes: heap_stats.current_usage,
            ram_total_bytes: heap_stats.size,
        }
    }
}

fn battery_percent_from_mv(millivolts: u32) -> u8 {
    if millivolts <= BATTERY_EMPTY_MV {
        return 0;
    }
    if millivolts >= BATTERY_FULL_MV {
        return 100;
    }

    let span = BATTERY_FULL_MV - BATTERY_EMPTY_MV;
    let used = millivolts - BATTERY_EMPTY_MV;
    ((used * 100) / span) as u8
}
