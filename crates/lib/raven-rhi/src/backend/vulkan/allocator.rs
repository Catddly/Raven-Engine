use gpu_allocator::vulkan::{Allocator as VulkanAllocator, AllocatorCreateDesc as VulkanAllocatorCreateDesc, 
    AllocationCreateDesc as VulkanAllocationCreateDesc, Allocation as VulkanAllocation};
use gpu_allocator::AllocatorDebugSettings as VulkanAllocatorDebugSettings;
use gpu_allocator::MemoryLocation as VulkanMemoryLocation;

pub type Allocator = VulkanAllocator;
pub type AllocatorCreateDesc = VulkanAllocatorCreateDesc;
pub type Allocation = VulkanAllocation;
pub type AllocationCreateDesc<'a> = VulkanAllocationCreateDesc<'a>;

pub type AllocatorDebugSettings = VulkanAllocatorDebugSettings;

/// Same as gpu_allocator::MemoryLocation but add Hash trait
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryLocation {
    /// The allocated resource is stored at an unknown memory location; let the driver decide what's the best location
    Unknown,
    /// Store the allocation in GPU only accessible memory - typically this is the faster GPU resource and this should be
    /// where most of the allocations live.
    GpuOnly,
    /// Memory useful for uploading data to the GPU and potentially for constant buffers
    CpuToGpu,
    /// Memory useful for CPU readback of data
    GpuToCpu,
}

pub fn to_inner_memory_location(loc: &MemoryLocation) -> VulkanMemoryLocation {
    match loc {
        MemoryLocation::Unknown => VulkanMemoryLocation::Unknown,
        MemoryLocation::GpuOnly => VulkanMemoryLocation::GpuOnly,
        MemoryLocation::CpuToGpu => VulkanMemoryLocation::CpuToGpu,
        MemoryLocation::GpuToCpu => VulkanMemoryLocation::GpuToCpu,
    }
}