/* SPDX-License-Identifier: Apache-2.0 */
/*
 * TSFFS compiled-in harness for libpldm's PLDM firmware-update package
 * decoder (decode_pldm_firmware_update_package, PLDM Type 5 / DSP0267).
 * Bare-metal ARM32 target: no OS, no libc - just enough runtime support
 * (memcpy/memcmp/memset/__assert_fail stubs below) for libpldm's decoder
 * path to run standalone on the AST2600's arm-cortex-a7 core.
 */
#include <stddef.h>
#include <stdint.h>

#include <libpldm/base.h>
#include <libpldm/firmware_update.h>

#include "tsffs.h"

#define TESTCASE_MAX_SIZE 4096

static uint8_t testcase[TESTCASE_MAX_SIZE];
static size_t testcase_size;

void* memcpy(void* dest, const void* src, size_t n) {
  unsigned char* d = (unsigned char*)dest;
  const unsigned char* s = (const unsigned char*)src;
  while (n--) *d++ = *s++;
  return dest;
}

int memcmp(const void* a, const void* b, size_t n) {
  const unsigned char* pa = (const unsigned char*)a;
  const unsigned char* pb = (const unsigned char*)b;
  while (n--) {
    if (*pa != *pb) return (int)*pa - (int)*pb;
    pa++;
    pb++;
  }
  return 0;
}

void* memset(void* dest, int value, size_t n) {
  unsigned char* d = (unsigned char*)dest;
  while (n--) *d++ = (unsigned char)value;
  return dest;
}

void __assert_fail(const char* assertion, const char* file, unsigned int line,
                    const char* function) {
  (void)assertion;
  (void)file;
  (void)line;
  (void)function;
  HARNESS_ASSERT();
  for (;;) {
  }
}

/* pldm_edac_crc32_validate is a weak symbol in libpldm/src/edac.c; override
 * it the same way libpldm's own tests/fuzz/fuzz.c does, so the decoder never
 * needs a real CRC32 implementation (irrelevant to the parsing logic we're
 * fuzzing) and always takes the "checksum ok" path. */
int pldm_edac_crc32_validate(uint32_t expected, const void* data,
                              size_t size) {
  (void)expected;
  (void)data;
  (void)size;
  return 0;
}

void _start(void) {
  struct pldm_package_format_pin pin;
  pldm_package_header_information_pad hdr;
  struct pldm_package pkg;
  struct pldm_package_firmware_device_id_record fdrec;
  struct pldm_package_downstream_device_id_record ddrec;
  struct pldm_package_component_image_information info;
  int rc;

  for (;;) {
    testcase_size = sizeof(testcase);
    HARNESS_START(testcase, &testcase_size);

    if (testcase_size > sizeof(testcase)) {
      testcase_size = sizeof(testcase);
    }

    pkg = (struct pldm_package){0};

    rc = decode_pldm_firmware_update_package(testcase, testcase_size, &pin,
                                              &hdr, &pkg, 0);
    if (rc < 0) {
      HARNESS_STOP();
      continue;
    }

    foreach_pldm_package_firmware_device_id_record(pkg, fdrec, rc) {
      (void)fdrec;
    }

    foreach_pldm_package_downstream_device_id_record(pkg, ddrec, rc) {
      struct pldm_descriptor desc;
      foreach_pldm_package_downstream_device_id_record_descriptor(pkg, ddrec,
                                                                   desc, rc) {
        (void)desc;
      }
    }

    foreach_pldm_package_component_image_information(pkg, info, rc) {
      (void)info;
    }

    HARNESS_STOP();
  }
}
