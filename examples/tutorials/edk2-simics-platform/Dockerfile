FROM ghcr.io/tianocore/containers/fedora-37-build:a0dd931@sha256:09843c8043412b49c36bd3daaa0660210b3bcd60a8417fe9f9e24f986cd345ff

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN dnf install -y clang llvm lld && \
     dnf clean all

ARG PROJECT=
ARG EDK2_REPO="https://github.com/cglosner/edk2.git"
ARG EDK2_REF="simics-sanitizer"
ARG EDK2_PLATFORMS_REPO_URL="https://github.com/cglosner/edk2-platforms.git"
ARG EDK2_PLATFORMS_BRANCH="simics-sanitizer"
ARG EDK2_NON_OSI_REPO_URL="https://github.com/cglosner/edk2-non-osi.git"
ARG EDK2_NON_OSI_BRANCH="simics-sanitizer"
ARG INTEL_FSP_REPO_URL="https://github.com/cglosner/FSP.git"
ARG INTEL_FSP_BRANCH="simics-sanitizer"

WORKDIR "$PROJECT"

RUN git clone "${EDK2_REPO}" edk2 && \
     git clone "${EDK2_PLATFORMS_REPO_URL}" edk2-platforms && \
     git clone "${EDK2_NON_OSI_REPO_URL}" edk2-non-osi && \
     git clone "${INTEL_FSP_REPO_URL}" FSP

ARG EDK2_HASH="${EDK2_REF}"
ARG EDK2_PLATFORMS_HASH="${EDK2_PLATFORMS_BRANCH}"
ARG EDK2_NON_OSI_HASH="${EDK2_NON_OSI_BRANCH}"
ARG INTEL_FSP_HASH="${INTEL_FSP_BRANCH}"

RUN git -C edk2 checkout "${EDK2_HASH}" && \
     git -C edk2 submodule update --init --depth 1 && \
     git -C edk2-platforms checkout "${EDK2_PLATFORMS_HASH}" && \
     git -C edk2-platforms submodule update --init --depth 1 && \
     git -C edk2-non-osi checkout "${EDK2_NON_OSI_HASH}" && \
     git -C edk2-non-osi submodule update --init --depth 1 && \
     git -C FSP checkout "${INTEL_FSP_HASH}" && \
     git -C FSP submodule update --init --depth 1

RUN sed -i \
       '/EfiBootManagerAddLoadOptionVariable/s/MAX_UINTN/0/' \
       edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/Library/BoardBdsHookLib/BoardBdsHookLib.c && \
     grep -q 'EfiBootManagerAddLoadOptionVariable (&NewOption, 0);' \
       edk2-platforms/Platform/Intel/SimicsOpenBoardPkg/Library/BoardBdsHookLib/BoardBdsHookLib.c

RUN sed -i \
       -e '/ASSERT (ShellInfoObject.NewEfiShellProtocol != NULL);/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: shell protocols installed\\n"));' \
       -e '/Status = ShellInitialize ();/i\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before ShellInitialize\\n"));' \
       -e '/Status = ShellInitialize ();/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after ShellInitialize: %r\\n", Status));' \
       -e '/Status = CommandInit ();/i\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before CommandInit\\n"));' \
       -e '/Status = CommandInit ();/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after CommandInit: %r\\n", Status));' \
       -e '/Status = ShellInitEnvVarList ();/i\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before ShellInitEnvVarList\\n"));' \
       -e '/Status = ShellInitEnvVarList ();/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after ShellInitEnvVarList: %r\\n", Status));' \
       -e '/Status = ProcessCommandLine ();/i\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before ProcessCommandLine\\n"));' \
       -e '/Status = ProcessCommandLine ();/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after ProcessCommandLine: %r\\n", Status));' \
       -e '/Status = ShellCommandCreateInitialMappingsAndPaths ();/i\      DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before ShellCommandCreateInitialMappingsAndPaths\\n"));' \
       -e '/Status = ShellCommandCreateInitialMappingsAndPaths ();/a\      DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after ShellCommandCreateInitialMappingsAndPaths: %r\\n", Status));' \
       -e '/Status = GetDevicePathsForImageAndFile (&ShellInfoObject.ImageDevPath, &ShellInfoObject.FileDevPath);/i\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before GetDevicePathsForImageAndFile\\n"));' \
       -e '/Status = GetDevicePathsForImageAndFile (&ShellInfoObject.ImageDevPath, &ShellInfoObject.FileDevPath);/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after GetDevicePathsForImageAndFile: %r\\n", Status));' \
       -e '/Status = RunCommand (L"map");/i\      DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before initial map command\\n"));' \
       -e '/Status = RunCommand (L"map");/a\      DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after initial map command: %r\\n", Status));' \
       -e '/Status = SetBuiltInAlias ();/i\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before SetBuiltInAlias\\n"));' \
       -e '/Status = SetBuiltInAlias ();/a\    DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after SetBuiltInAlias: %r\\n", Status));' \
       -e '/Status = DoStartupScript (ShellInfoObject.ImageDevPath, ShellInfoObject.FileDevPath);/i\        DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before DoStartupScript\\n"));' \
       -e '/Status = DoStartupScript (ShellInfoObject.ImageDevPath, ShellInfoObject.FileDevPath);/a\        DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after DoStartupScript: %r\\n", Status));' \
       -e '/Status = DoShellPrompt ();/i\          DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: before DoShellPrompt\\n"));' \
       -e '/Status = DoShellPrompt ();/a\          DEBUG ((DEBUG_INFO, "TSFFS ShellTrace: after DoShellPrompt: %r\\n", Status));' \
       edk2/ShellPkg/Application/Shell/Shell.c && \
     grep -q 'TSFFS ShellTrace: before ShellInitialize' \
       edk2/ShellPkg/Application/Shell/Shell.c

RUN sed -i \
       -e '/gST->ConOut->QueryMode (gST->ConOut, gST->ConOut->Mode->Mode, &Column, &Row);/i\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: before QueryMode ConIn=%p ConInHandle=%p ConOut=%p ConOutHandle=%p\\n", gST->ConIn, gST->ConsoleInHandle, gST->ConOut, gST->ConsoleOutHandle));' \
       -e '/gST->ConOut->QueryMode (gST->ConOut, gST->ConOut->Mode->Mode, &Column, &Row);/a\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: after QueryMode Column=%Lu Row=%Lu\\n", (UINT64)Column, (UINT64)Row));' \
       -e '/CmdLine    = AllocateZeroPool (BufferSize);/a\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: allocated command buffer size=%Lu\\n", (UINT64)BufferSize));' \
       -e '/CurDir = ShellInfoObject.NewEfiShellProtocol->GetEnv (L"cwd");/a\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: cwd=%s\\n", CurDir == NULL ? L"(null)" : CurDir));' \
       -e '/gST->ConOut->SetCursorPosition (gST->ConOut, 0, gST->ConOut->Mode->CursorRow);/i\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: before SetCursorPosition row=%Lu\\n", (UINT64)gST->ConOut->Mode->CursorRow));' \
       -e '/gST->ConOut->SetCursorPosition (gST->ConOut, 0, gST->ConOut->Mode->CursorRow);/a\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: after SetCursorPosition\\n"));' \
       -e '/ShellPrintHiiEx (-1, -1, NULL, STRING_TOKEN (STR_SHELL_CURDIR), ShellInfoObject.HiiHandle, CurDir);/i\    DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: before ShellPrintHiiEx cwd prompt\\n"));' \
       -e '/ShellPrintHiiEx (-1, -1, NULL, STRING_TOKEN (STR_SHELL_CURDIR), ShellInfoObject.HiiHandle, CurDir);/a\    DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: after ShellPrintHiiEx cwd prompt\\n"));' \
       -e '/ShellPrintHiiEx (-1, -1, NULL, STRING_TOKEN (STR_SHELL_SHELL), ShellInfoObject.HiiHandle);/i\    DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: before ShellPrintHiiEx shell prompt\\n"));' \
       -e '/ShellPrintHiiEx (-1, -1, NULL, STRING_TOKEN (STR_SHELL_SHELL), ShellInfoObject.HiiHandle);/a\    DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: after ShellPrintHiiEx shell prompt\\n"));' \
       -e '/Status = ShellInfoObject.NewEfiShellProtocol->ReadFile (ShellInfoObject.NewShellParametersProtocol->StdIn, &BufferSize, CmdLine);/i\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: before StdIn ReadFile BufferSize=%Lu StdIn=%p\\n", (UINT64)BufferSize, ShellInfoObject.NewShellParametersProtocol->StdIn));' \
       -e '/Status = ShellInfoObject.NewEfiShellProtocol->ReadFile (ShellInfoObject.NewShellParametersProtocol->StdIn, &BufferSize, CmdLine);/a\  DEBUG ((DEBUG_INFO, "TSFFS PromptTrace: after StdIn ReadFile Status=%r BufferSize=%Lu CmdLine=%s\\n", Status, (UINT64)BufferSize, CmdLine));' \
       edk2/ShellPkg/Application/Shell/Shell.c && \
     sed -i \
       -e '/gST->ConOut->QueryMode (gST->ConOut, gST->ConOut->Mode->Mode, &TotalColumn, &TotalRow);/a\  DEBUG ((DEBUG_INFO, "TSFFS StdinTrace: entry BufferSize=%Lu TotalColumn=%Lu TotalRow=%Lu ConIn=%p WaitForKey=%p\\n", (UINT64)*BufferSize, (UINT64)TotalColumn, (UINT64)TotalRow, gST->ConIn, gST->ConIn->WaitForKey));' \
       -e '/gBS->WaitForEvent (1, &gST->ConIn->WaitForKey, &EventIndex);/i\    DEBUG ((DEBUG_INFO, "TSFFS StdinTrace: before WaitForEvent\\n"));' \
       -e '/gBS->WaitForEvent (1, &gST->ConIn->WaitForKey, &EventIndex);/a\    DEBUG ((DEBUG_INFO, "TSFFS StdinTrace: after WaitForEvent EventIndex=%Lu\\n", (UINT64)EventIndex));' \
       -e '/Status = gST->ConIn->ReadKeyStroke (gST->ConIn, &Key);/a\    DEBUG ((DEBUG_INFO, "TSFFS StdinTrace: after ReadKeyStroke Status=%r Scan=%Lu Char=0x%04x\\n", Status, (UINT64)Key.ScanCode, Key.UnicodeChar));' \
       edk2/ShellPkg/Application/Shell/FileHandleWrappers.c && \
     grep -q 'TSFFS StdinTrace: before WaitForEvent' \
       edk2/ShellPkg/Application/Shell/FileHandleWrappers.c

ENV EDK_TOOLS_PATH="${PROJECT}/edk2/BaseTools/"
ENV PACKAGES_PATH="${PROJECT}/edk2:${PROJECT}/edk2-platforms:${PROJECT}/edk2-non-osi"
ENV WORKSPACE="${PROJECT}"
ENV CLANGSAN_BIN="/usr/bin/"

RUN source edk2/edksetup.sh && \
     make -C edk2/BaseTools/

WORKDIR "${PROJECT}/edk2-platforms/Platform/Intel"

# Tutorial-local TSFFS patch for DxeLogoLib is intentionally disabled for now.
# The ASAN platform branches are built as-is.

# Build SimicsOpenBoardPkg
RUN source "${PROJECT}/edk2/edksetup.sh" && \
     python build_bios.py -p BoardX58Ich10 -d -t CLANGSAN
