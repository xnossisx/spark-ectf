file target/thumbv7em-none-eabihf/debug/spark-decoder

target extended-remote :3333

# print demangled symbols
set print asm-demangle on

# set backtrace limit to not have infinite backtrace loops
set backtrace limit 32

# detect unhandled exceptions, hard faults and panics
break DefaultHandler
break HardFault
# # run the next few lines so the panic message is printed immediately
# # the number needs to be adjusted for your panic handler
commands $bpnum
next 4
end

# *try* to stop at the user entry point (it might be gone due to inlining)
break main

# enable semihosting
monitor arm semihosting enable

# load the program
load

# start the process but immediately halt the processor
stepi
