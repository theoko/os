/* AArch64 Limine-compatible ELF. Kept separate from x86_64 so the existing
 * UTM image remains untouched while the ARM edition comes online. */
OUTPUT_FORMAT(elf64-littleaarch64)
ENTRY(kmain)

PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    . = 0xffffffff80000000;

    .text : { *(.text .text.*) } :text
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .rodata : { *(.rodata .rodata.*) } :rodata
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .data : {
        *(.data .data.*)
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    } :data
    .bss : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) }
}
