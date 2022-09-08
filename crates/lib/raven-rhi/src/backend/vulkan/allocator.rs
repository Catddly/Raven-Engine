use gpu_allocator::vulkan::{Allocator as VulkanAllocator, AllocatorCreateDesc as VulkanAllocatorCreateDesc, 
    AllocationCreateDesc as VulkanAllocationCreateDesc, Allocation as VulkanAllocation};
use gpu_allocator::AllocatorDebugSettings as VulkanAllocatorDebugSettings;
use gpu_allocator::MemoryLocation as VulkanMemoryLocation;

pub type Allocator = VulkanAllocator;
pub type AllocatorCreateDesc = VulkanAllocatorCreateDesc;
pub type Allocation = VulkanAllocation;
pub type AllocationCreateDesc<'a> = VulkanAllocationCreateDesc<'a>;

pub type AllocatorDebugSettings = VulkanAllocatorDebugSettings;

pub type MemoryLocation = VulkanMemoryLocation;