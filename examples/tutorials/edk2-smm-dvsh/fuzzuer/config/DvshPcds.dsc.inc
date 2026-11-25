# SMM page-table protection for this step.
[PcdsFixedAtBuild.X64]
!if $(DVSH_PT_PROTECT) == TRUE
  gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmRestrictedMemoryAccess|TRUE
!else
  gUefiCpuPkgTokenSpaceGuid.PcdCpuSmmRestrictedMemoryAccess|FALSE
!endif
