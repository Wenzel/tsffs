# Step toggles (DVSH_PT_PROTECT, DVSH_HEAP_GUARD, DVSH_UBSAN: TRUE or FALSE).
# build.sh generates this file for the step being built; build_bios.py does
# not forward environment variables to the EDK2 build, so a file is needed.
!include $(BOARD_PKG)/BoardX58Ich10/DvshStep.dsc.inc

# DVSH is full of deliberate bugs; keep GCC from rejecting them (-Werror) at build time.
  DEFINE DVSH_WARN_FLAGS = -Wno-error=dangling-pointer -Wno-error=use-after-free -Wno-error=free-nonheap-object -Wno-error=stringop-overflow -Wno-error=array-bounds -Wno-error=maybe-uninitialized -Wno-error=uninitialized -Wno-error=return-local-addr -Wno-error=div-by-zero -Wno-error=shift-count-overflow -Wno-error=nonnull -Wno-error=unused-but-set-variable

# Full UBSan (-fsanitize=undefined), applied to DvshSmi only and in trap mode:
# a failed check is a #UD and no UBSan runtime is needed.
!if $(DVSH_UBSAN) == TRUE
  DEFINE DVSH_CC_FLAGS = $(DVSH_WARN_FLAGS) -fsanitize=undefined -fsanitize-undefined-trap-on-error
!else
  DEFINE DVSH_CC_FLAGS = $(DVSH_WARN_FLAGS)
!endif
