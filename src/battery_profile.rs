//! Pure profile/estimator preparation. No IO, runtime provider integration, or
//! voltage-to-capacity fallback. Imported numbers are not selection evidence.
//! Interpolation remains deliberately unavailable pending source/license audit.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileError {
    Identity,
    Dimensions,
    TemperatureOrder,
    PercentOrder,
    Value,
}

/// Row-major tables, units explicit. Owned import boundary, not a DTS parser.
/// Callers must separately establish that this profile fits the actual battery.
#[derive(Clone, Debug)]
pub struct ProfileImport {
    pub identity: String,
    pub source_receipt: String,
    pub temperatures_celsius: Vec<i32>,
    pub ocv_percent_rows: Vec<u8>,
    pub ocv_millivolts: Vec<u32>,
    pub fcc_milliamp_hours: Vec<u32>,
    pub rbatt_percent_rows: Vec<u8>,
    pub rbatt_scale_percent: Vec<u32>,
    pub acc_temperatures_celsius: Vec<i32>,
    pub acc_current_milliamps: Vec<u32>,
    pub acc_milliamp_hours: Vec<u32>,
    pub nominal_fcc_milliamp_hours: u32,
    pub default_rbatt_milliohms: u32,
    pub capacitive_rbatt_milliohms: u32,
}

#[derive(Clone, Debug)]
pub struct ValidatedProfile(ProfileImport);
impl ValidatedProfile {
    /// Structural validation only: neither profile selection nor calibration.
    pub fn import(data: ProfileImport) -> Result<Self, ProfileError> {
        if data.identity.trim().is_empty() || data.source_receipt.trim().is_empty() {
            return Err(ProfileError::Identity);
        }
        let columns = data.temperatures_celsius.len();
        if columns < 2
            || data.ocv_percent_rows.len() < 2
            || data.rbatt_percent_rows.len() < 2
            || data.ocv_percent_rows.len().checked_mul(columns) != Some(data.ocv_millivolts.len())
            || data.rbatt_percent_rows.len().checked_mul(columns)
                != Some(data.rbatt_scale_percent.len())
            || data.fcc_milliamp_hours.len() != columns
            || data.acc_temperatures_celsius.len() < 2
            || data.acc_current_milliamps.len() < 2
            || data
                .acc_current_milliamps
                .len()
                .checked_mul(data.acc_temperatures_celsius.len())
                != Some(data.acc_milliamp_hours.len())
        {
            return Err(ProfileError::Dimensions);
        }
        if !data.temperatures_celsius.windows(2).all(|w| w[0] < w[1])
            || !data
                .acc_temperatures_celsius
                .windows(2)
                .all(|w| w[0] < w[1])
        {
            return Err(ProfileError::TemperatureOrder);
        }
        for rows in [&data.ocv_percent_rows, &data.rbatt_percent_rows] {
            if rows[0] != 100 || !rows.windows(2).all(|w| w[0] > w[1]) {
                return Err(ProfileError::PercentOrder);
            }
        }
        if data.ocv_percent_rows.last() != Some(&0) {
            return Err(ProfileError::PercentOrder);
        }
        if !data.acc_current_milliamps.windows(2).all(|w| w[0] < w[1]) {
            return Err(ProfileError::Value);
        }
        if data.ocv_millivolts.contains(&0)
            || data.fcc_milliamp_hours.contains(&0)
            || data.rbatt_scale_percent.contains(&0)
            || data.nominal_fcc_milliamp_hours == 0
            || data.default_rbatt_milliohms == 0
        {
            return Err(ProfileError::Value);
        }
        // Do not impose guessed physical ranges, cross-column monotonicity or
        // smooth away plateaus. Exact algorithm must handle the exact table.
        Ok(Self(data))
    }
    pub fn data(&self) -> &ProfileImport {
        &self.0
    }
}

/// Trusted qualifier evidence, NOT a way to qualify raw voltage_now. Exact
/// producers and evidence requirements await the capacity/validity audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationReceipt {
    pub source: String,
    pub qualification: String,
    pub observed_monotonic_ms: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedOcv {
    pub microvolts: u32,
    pub receipt: QualificationReceipt,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedTemperature {
    pub deci_celsius: i32,
    pub receipt: QualificationReceipt,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EstimationFailure {
    UnverifiedAlgorithm,
    InvalidQualification,
    InvalidTime,
    Stale,
}
/// This scaffold never returns capacity. No 25C default, current guess, or
/// implicit conversion from BatterySample/voltage_now exists.
pub fn estimate_capacity(
    _profile: &ValidatedProfile,
    ocv: &QualifiedOcv,
    temperature: &QualifiedTemperature,
    now_ms: u64,
    max_age_ms: u64,
) -> Result<u8, EstimationFailure> {
    for receipt in [&ocv.receipt, &temperature.receipt] {
        if receipt.source.trim().is_empty() || receipt.qualification.trim().is_empty() {
            return Err(EstimationFailure::InvalidQualification);
        }
        let age = now_ms
            .checked_sub(receipt.observed_monotonic_ms)
            .ok_or(EstimationFailure::InvalidTime)?;
        if age > max_age_ms {
            return Err(EstimationFailure::Stale);
        }
    }
    Err(EstimationFailure::UnverifiedAlgorithm)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Synthetic shape-validation fixture, NOT Q2805 or interpolation evidence.
    fn fixture() -> ProfileImport {
        ProfileImport {
            identity: "synthetic".into(),
            source_receipt: "test-only".into(),
            temperatures_celsius: vec![-20, 25],
            ocv_percent_rows: vec![100, 0],
            ocv_millivolts: vec![4300, 4300, 3000, 3000],
            fcc_milliamp_hours: vec![2000, 2000],
            rbatt_percent_rows: vec![100, 1],
            rbatt_scale_percent: vec![100; 4],
            acc_temperatures_celsius: vec![-20, 25],
            acc_current_milliamps: vec![0, 250],
            acc_milliamp_hours: vec![2000; 4],
            nominal_fcc_milliamp_hours: 2000,
            default_rbatt_milliohms: 222,
            capacitive_rbatt_milliohms: 50,
        }
    }
    #[test]
    fn rejects_malformed_import_without_normalizing() {
        assert!(ValidatedProfile::import(fixture()).is_ok());
        let mut p = fixture();
        p.ocv_millivolts.pop();
        assert_eq!(
            ValidatedProfile::import(p).unwrap_err(),
            ProfileError::Dimensions
        );
        let mut p = fixture();
        p.temperatures_celsius.reverse();
        assert_eq!(
            ValidatedProfile::import(p).unwrap_err(),
            ProfileError::TemperatureOrder
        );
        let mut p = fixture();
        p.ocv_percent_rows = vec![100, 1];
        assert_eq!(
            ValidatedProfile::import(p).unwrap_err(),
            ProfileError::PercentOrder
        );
        let mut p = fixture();
        p.rbatt_percent_rows = vec![100, 100];
        assert_eq!(
            ValidatedProfile::import(p).unwrap_err(),
            ProfileError::PercentOrder
        );
        let mut p = fixture();
        p.fcc_milliamp_hours[0] = 0;
        assert_eq!(
            ValidatedProfile::import(p).unwrap_err(),
            ProfileError::Value
        );
        let mut p = fixture();
        p.source_receipt.clear();
        assert_eq!(
            ValidatedProfile::import(p).unwrap_err(),
            ProfileError::Identity
        );
    }
    #[test]
    fn no_capacity_even_with_explicit_fresh_inputs() {
        let profile = ValidatedProfile::import(fixture()).unwrap();
        let receipt = QualificationReceipt {
            source: "synthetic-test".into(),
            qualification: "not-hardware".into(),
            observed_monotonic_ms: 100,
        };
        let mut ocv = QualifiedOcv {
            microvolts: 3800000,
            receipt: receipt.clone(),
        };
        let temperature = QualifiedTemperature {
            deci_celsius: 250,
            receipt,
        };
        assert_eq!(
            estimate_capacity(&profile, &ocv, &temperature, 100, 1000),
            Err(EstimationFailure::UnverifiedAlgorithm)
        );
        assert_eq!(
            estimate_capacity(&profile, &ocv, &temperature, 99, 1000),
            Err(EstimationFailure::InvalidTime)
        );
        assert_eq!(
            estimate_capacity(&profile, &ocv, &temperature, 1101, 1000),
            Err(EstimationFailure::Stale)
        );
        ocv.receipt.qualification.clear();
        assert_eq!(
            estimate_capacity(&profile, &ocv, &temperature, 100, 1000),
            Err(EstimationFailure::InvalidQualification)
        );
    }
}
