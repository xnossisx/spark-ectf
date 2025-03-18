from Crypto.Cipher import AES
import random


data = random.Random("hola").randbytes(128)

key = random.Random("hi").randbytes(32)

cipher = AES.new(key[:16], AES.MODE_OFB, iv=key[16:])

ct_bytes = cipher.encrypt(data)
print(len(cipher.iv), len(ct_bytes))