#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0

from os import environ
from pyftdi.ftdi import Ftdi
from pyftdi.misc import pretty_size
from pyftdi.spi import SpiController, SpiPort
from spiflash.serialflash import SerialFlashManager, _Gen25FlashDevice, SerialFlash, SerialFlashUnknownJedec
from typing import Iterable, Optional, Tuple, Union

class Puya25FlashDevice(_Gen25FlashDevice):
    '''Caution: only reads have been tested on Puya Flash devices'''
    JEDEC_ID = 0x85
    DEVICES = {0x60: 'P25D80SH'}
    SIZES = {0x14: 1 << 20 } # 8Mbit
    SPI_FREQ_MAX = 55 ## 55 Mhz max for READ; 104 Mhz for Fast Read
    #TIMINGS = {'page': (0.0015, 0.003),  # 1.5/3 ms
    #           'subsector': (0.016, 0.030),  # 16/30 ms
    #           'hsector': (0.016, 0.030),  # 16/30 ms
    #           'sector': (0.016, 0.030),  # 16/30 ms
    #           'bulk': (0.080, 0.180),  # 80 ms / 180 s
    #           }
    FEATURES = (SerialFlash.FEAT_SECTERASE |
                SerialFlash.FEAT_SUBSECTERASE |
                SerialFlash.FEAT_HSECTERASE)

    def __init__(self, spi, jedec):
        super(Puya25FlashDevice, self).__init__(spi)
        if not Puya25FlashDevice.match(jedec):
            raise SerialFlashUnknownJedec(jedec)
        device, capacity = jedec[1:3]
        self._device = self.DEVICES[device]
        self._size = Puya25FlashDevice.SIZES[capacity]
    
    def __str__(self):
        return 'Puya %s %s' % \
            (self._device, pretty_size(self._size, lim_m=1 << 20))

    def set_spi_frequency(self, freq: Optional[float] = None) -> None:
        default_freq = self.SPI_FREQ_MAX*1E06
        freq = min(default_freq, freq) if freq else default_freq
        self._spi.set_frequency(freq)

ftdi_url = environ.get('FTDI_DEVICE', 'ftdi://ftdi:2232:TG1001a8/2')
Ftdi.show_devices()
# If our Flash was supported by SerialFlashManager, we could use:
#flash=SerialFlashManager.get_flash_device(ftdi_url)
ctrl = SpiController(cs_count=1)
ctrl.configure(ftdi_url)
spi = ctrl.get_port(0, None)
jedec  = SerialFlashManager.read_jedec_id(spi)
print("JEDEC ID: %s" % ' '.join('%02X' % b for b in jedec))
if Puya25FlashDevice.match(jedec):
    flash = Puya25FlashDevice(spi, jedec)
    # can do faster, but let's be conservative for data integrity
    flash.set_spi_frequency(8*1E6) # 8 MHz
    print("Flash device: %s @ SPI freq %0.1f MHz" % (flash, flash.spi_frequency/1E6))
    f=open("data.bin","wb")
    f.write(flash.read(0,len(flash)))
    f.close()
else:
    print("Couldn't find Puya Flash device")
