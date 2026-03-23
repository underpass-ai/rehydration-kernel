pub mod v1alpha1 {
    #![allow(clippy::all)]
    #![allow(missing_docs)]

    tonic::include_proto!("underpass.rehydration.kernel.v1alpha1");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("rehydration_kernel_v1alpha1_descriptor");
}

pub mod v1beta1 {
    #![allow(clippy::all)]
    #![allow(missing_docs)]

    tonic::include_proto!("underpass.rehydration.kernel.v1beta1");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("rehydration_kernel_v1beta1_descriptor");
}

pub mod fleet_context_v1 {
    #![allow(clippy::all)]
    #![allow(missing_docs)]

    tonic::include_proto!("fleet.context.v1");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("fleet_context_v1_descriptor");
}

#[cfg(test)]
mod asyncapi_contract_tests;
#[cfg(test)]
mod compatibility_contract_tests;
#[cfg(test)]
mod kernel_contract_tests;
#[cfg(test)]
mod kernel_v1beta1_contract_tests;
#[cfg(test)]
mod reference_fixture_contract_tests;
