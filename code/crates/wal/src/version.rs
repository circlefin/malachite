/// Version identifier for the Write-Ahead Log (WAL) format
///
/// Currently only supports version 1 (V1) of the format.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    /// Version 1 of the WAL format
    V1 = 1,
}

impl Version {
    /// Converts the WAL version to its raw u32 representation
    ///
    /// # Returns
    /// The version number as a u32 value
    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

impl TryFrom<u32> for Version {
    type Error = ();

    /// Attempts to convert a u32 into a WalVersion
    ///
    /// # Arguments
    /// * `value` - The u32 value to convert
    ///
    /// # Returns
    /// * `Ok(WalVersion)` - If the value represents a valid version
    /// * `Err(())` - If the value does not correspond to any known version
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            _ => Err(()),
        }
    }
}
