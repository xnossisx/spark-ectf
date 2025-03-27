python3 -m ectf25_design.gen_secrets --force ./global.secrets 1 4294967295 4294967290 4294967285 1000 40000 600000 2000000000 2866811428 770889830 1361404487 28377511 3281870776
docker run --rm -v ./build_out:/out -v ./decoder:/decoder -v ./global.secrets:/global.secrets -e DECODER_ID=0xdeadbeef build-decoder
python3 -m ectf25_design.gen_subscription --force ./global.secrets subscriptions/sub1.bin 0xdeadbeef 5 1842673849973240 1
read -p "Press enter to continue when the device is blinking red and flash mode is on"
# /home/bruberu/prgm/MaximSDK/Tools/OpenOCD/openocd -s scripts/ -f interface/cmsis-dap.cfg -f target/max78000.cfg -c "init; reset halt; max32xxx mass_erase 0;
# program decoder/insecure.bin verify 0x10000000; program decoder/5a.bin verify 0x10002000; program build_out/max78000.bin 0x1000E000 verify reset exit "
python3 -m ectf25.utils.flash build_out/max78000.bin /dev/ttyACM0
sleep 0.2s
sleep 0s
python -m ectf25.tv.subscribe sub.bin /dev/ttyACM0
python3 -m ectf25.utils.tester -s ./global.secrets --port /dev/ttyACM0 json frames/x_c0123.json
