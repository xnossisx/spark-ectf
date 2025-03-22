openocd -s scripts/ -f interface/cmsis-dap.cfg -f target/max78000.cfg -c "init; reset halt; max32xxx mass_erase 0;
 program decoder/insecure.bin verify 0x10000000; program decoder/5a.bin verify 0x10002000; program build_out/max78000.bin 0x1000E000 verify reset exit "
sleep 0.5
python3 -m ectf25.utils.tester -s ./global.secrets --port /dev/ttyACM0 rand -c 0
