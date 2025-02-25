#!/bin/bash
if [ "$#" -eq 0 ] ; 
then
  echo -e "No argument."
  echo -e "Write either debug or release."
  exit 1
fi
cargo build
cargo objcopy --profile dev -- -O binary app.bin
openocd -s scripts/ -f interface/cmsis-dap.cfg -f target/max78000.cfg -c "init; reset halt; max32xxx mass_erase 0;
 program insecure.bin verify 0x10000000; program 5a.bin verify 0x10002000; program app.bin 0x1000E000 verify reset exit "