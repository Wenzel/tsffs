do_patch[postfuncs] += "remove_uart5_dw_apb_workaround"
do_patch[postfuncs] += "disable_n25q512_flag_status_register"

# The board dts overrides uart5's "compatible" from the generic "ns16550a" to
# "snps,dw-apb-uart" as an "A0 workaround". The snps,dw-apb-uart driver
# probes Synopsys DesignWare-specific extended UART registers (beyond the
# standard 16550 register window) that the target Simics AST2600 model does
# not implement, causing an unmapped write and a watchdog-triggered reboot
# loop before Linux ever reaches userspace. Reverting to the generic
# ns16550a compatible (by deleting the override lines, leaving an empty
# &uart5 {}; node) avoids touching those extended registers. This only
# affects the Simics target build of this PoC, not real A0 hardware.
remove_uart5_dw_apb_workaround() {
    sed -i \
        -e '/\/\/ Workaround for A0/d' \
        -e '/compatible = "snps,dw-apb-uart"/d' \
        "${S}/arch/arm/boot/dts/aspeed/aspeed-ast2600-evb.dts"
}

# The BMC boot flash is JEDEC-identified as a Micron n25q512ax3 (64MB), which
# the kernel's micron-st.c driver marks USE_FSR: after every erase/write it
# polls the Read Flag Status Register (opcode 0x70) instead of the plain
# status register. The target Simics generic_spi_flash_ext model does not
# implement that opcode, so every JFFS2 rwfs write spins forever reading an
# always-unimplemented register, hanging boot before userspace comes up.
# Winbond-family chips of the same size don't use FSR at all, but swapping
# the JEDEC ID outright breaks u-boot's own SPI flash probe (its detection
# also depends on the ID/size pairing). Instead, drop just the USE_FSR flag
# for this specific chip ID, keeping the Micron identity both u-boot and
# Linux already detect correctly. This only affects the Simics target build
# of this PoC; real n25q512ax3 hardware still benefits from FSR-based status
# polling, so this is not something to send upstream.
disable_n25q512_flag_status_register() {
    sed -i '/\.id = SNOR_ID(0x20, 0xba, 0x20),/,/},/ s/\.mfr_flags = USE_FSR,//' \
        "${S}/drivers/mtd/spi-nor/micron-st.c"
}
