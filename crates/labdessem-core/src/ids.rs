macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub usize);

        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value)
            }
        }
    };
}

define_id!(SubmarketId);
define_id!(BusId);
define_id!(BranchId);
define_id!(ThermalPlantId);
define_id!(ThermalUnitId);
define_id!(HydroPlantId);
define_id!(HydroGroupId);
define_id!(HydroUnitId);
define_id!(PumpingPlantId);
define_id!(RenewablePlantId);
