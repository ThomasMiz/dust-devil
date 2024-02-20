use std::{
    collections::VecDeque,
    ops::{Add, AddAssign},
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageMeasure {
    pub sent: u64,
    pub received: u64,
}

impl UsageMeasure {
    pub const fn new(sent: u64, received: u64) -> Self {
        Self { sent, received }
    }

    pub const fn zero() -> Self {
        Self::new(0, 0)
    }

    pub const fn sum(&self) -> u64 {
        self.sent + self.received
    }
}

impl Add for UsageMeasure {
    type Output = UsageMeasure;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            sent: self.sent + rhs.sent,
            received: self.received + rhs.received,
        }
    }
}

impl AddAssign for UsageMeasure {
    fn add_assign(&mut self, rhs: Self) {
        self.sent += rhs.sent;
        self.received += rhs.received;
    }
}

pub struct UsageTracker {
    history_by_second: VecDeque<UsageMeasure>,
    history_start_timestamp: i64,
    history_by_unit: VecDeque<UsageMeasure>,
    history_by_unit_start_timestamp: i64,
    unit_size_seconds: i64,
}

impl UsageTracker {
    pub fn new(max_history_seconds: usize) -> Self {
        let timestamp_now = time::OffsetDateTime::now_utc().unix_timestamp();

        let mut history_by_second = VecDeque::with_capacity(max_history_seconds);
        history_by_second.push_front(UsageMeasure::zero());

        Self {
            history_by_second,
            history_start_timestamp: timestamp_now,
            history_by_unit: VecDeque::new(),
            history_by_unit_start_timestamp: timestamp_now,
            unit_size_seconds: 1,
        }
    }

    pub fn record_usage(&mut self, timestamp: i64, bytes_sent: u64, bytes_received: u64) {
        let mut timestamp_index = match timestamp - self.history_start_timestamp {
            v if v < 0 => return,
            v => v as usize,
        };

        let new_seconds_count = (timestamp_index + 1).saturating_sub(self.history_by_second.len());

        let remove_count = (self.history_by_second.len() + new_seconds_count).saturating_sub(self.history_by_second.capacity());
        self.history_start_timestamp += remove_count as i64;
        timestamp_index -= remove_count;
        for _ in 0..remove_count {
            self.history_by_second.pop_front();
        }

        for _ in 0..new_seconds_count {
            self.history_by_second.push_back(UsageMeasure::zero());
        }

        let measure = &mut self.history_by_second[timestamp_index];
        measure.sent += bytes_sent;
        measure.received += bytes_received;

        if !self.history_by_unit.is_empty() {
            while self.history_by_unit_start_timestamp + self.unit_size_seconds <= self.history_start_timestamp {
                self.history_by_unit_start_timestamp += self.unit_size_seconds;
                self.history_by_unit.pop_front();
            }

            let history_end_timestamp = self.history_start_timestamp + self.history_by_second.len() as i64;
            let mut history_by_unit_end_timestamp =
                (history_end_timestamp + self.unit_size_seconds - 1) / self.unit_size_seconds * self.unit_size_seconds;

            while history_by_unit_end_timestamp < history_end_timestamp {
                history_by_unit_end_timestamp += self.unit_size_seconds;
                self.history_by_unit.push_back(UsageMeasure::zero());
            }

            let timestamp_index = (timestamp - self.history_by_unit_start_timestamp) / self.unit_size_seconds;
            let measure = &mut self.history_by_second[timestamp_index as usize];
            measure.sent += bytes_sent;
            measure.received += bytes_received;
        }
    }

    pub fn set_unit_size(&mut self, unit_size_seconds: i64) {
        self.history_by_unit.clear();
        self.unit_size_seconds = unit_size_seconds;
        if self.unit_size_seconds <= 1 {
            return;
        }

        self.history_by_unit_start_timestamp = self.history_start_timestamp / unit_size_seconds * unit_size_seconds;

        let history_end_timestamp = self.history_start_timestamp + self.history_by_second.len() as i64;
        let history_by_unit_end_timestamp = (history_end_timestamp + unit_size_seconds - 1) / unit_size_seconds * unit_size_seconds;

        let total_units = (history_by_unit_end_timestamp - self.history_by_unit_start_timestamp) / unit_size_seconds;
        for _ in 0..total_units {
            self.history_by_unit.push_back(UsageMeasure::zero());
        }

        for (i, measure) in self.history_by_second.iter().enumerate() {
            let timestamp = self.history_start_timestamp + i as i64;
            let timestamp_index = (timestamp - self.history_by_unit_start_timestamp) / unit_size_seconds;
            self.history_by_unit[timestamp_index as usize] += *measure;
        }
    }

    pub fn get_latest_timestamp(&self) -> i64 {
        self.history_start_timestamp + self.history_by_second.len() as i64
    }

    pub fn get_usage_by_unit(&mut self, unit_size_seconds: i64) -> &VecDeque<UsageMeasure> {
        if unit_size_seconds != self.unit_size_seconds {
            self.set_unit_size(unit_size_seconds);
        }

        if self.history_by_unit.is_empty() {
            &self.history_by_second
        } else {
            &self.history_by_unit
        }
    }
}
