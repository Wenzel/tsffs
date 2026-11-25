# SMM memory protections under test. The two below are mutually exclusive on
# X64 (see PcdCpuSmmRestrictedMemoryAccess in UefiCpuPkg.dec).
[PcdsFixedAtBuild.X64]
!if $(DVSH_PT_PROTECT) == TRUE
  gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmRestrictedMemoryAccess|TRUE
!else
  gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmRestrictedMemoryAccess|FALSE
!endif
!if $(DVSH_HEAP_GUARD) == TRUE
  # BIT2|BIT3: SMM page + pool guard. Pool type 0x5E includes
  # EfiRuntimeServicesData, which is what SMM AllocatePool() uses.
  gEfiMdeModulePkgTokenSpaceGuid.PcdHeapGuardPropertyMask|0x0F
  gEfiMdeModulePkgTokenSpaceGuid.PcdHeapGuardPoolType|0x5E
  gEfiMdeModulePkgTokenSpaceGuid.PcdHeapGuardPageType|0x1E
!endif
