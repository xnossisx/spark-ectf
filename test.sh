x="$(docker image ls | grep build-ectf-decoder-spark)"
if [[ $x == "" ]]; then
  docker build -t build-ectf-decoder-spark decoder
fi

python3 -m ectf25_design.gen_secrets --force ./global.secrets 1 4294967295 4294967290 4294967285 1000 40000 600000 2000000000 2866811428 770889830 1361404487 28377511 3281870776
docker run --rm -v ./build_out:/out -v ./decoder:/decoder -v ./global.secrets:/global.secrets -e DECODER_ID=0xdeadbeef build-ectf-decoder-spark
openocd -s scripts/ -f interface/cmsis-dap.cfg -f target/max78000.cfg -c "init; reset halt; max32xxx mass_erase 0;
 program decoder/insecure.bin verify 0x10000000; program decoder/5a.bin verify 0x10002000; program build_out/max78000.bin 0x1000E000 verify reset exit "
sleep 0.2s
python3 -m ectf25_design.gen_subscription --force ./global.secrets sub.bin 0xdeadbeef 20 110000 1
sleep 0s
python -m ectf25.tv.subscribe sub.bin /dev/ttyACM0
python3 -m ectf25.utils.tester -s ./global.secrets --port /dev/ttyACM0 json frames/x_c0123.json

