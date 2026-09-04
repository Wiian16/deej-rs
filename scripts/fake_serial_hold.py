from typing import Iterable
import os
import time
import pty
import itertools

# Value to hold sliders at
SLIDER_VALUE = 500
# Number of sliders to emulate
NUM_SLIDERS = 5
# Delay in seconds between ouputting each line
STEP_DELAY = 0.01

def gen_slider_line(value: int, sliders: int) -> bytes:
    return ("|".join([str(value)] * sliders) + "\n").encode()


def play_sequence(fd: int, values: Iterable[int], sliders: int, delay: float):
    """Write one slider-state line per value in `values`, sleeping `delay` between writes."""
    for value in values:
        os.write(fd, gen_slider_line(value, sliders))
        time.sleep(delay)

def main(): 
    master_fd, slave_fd = pty.openpty()
    print("Emulated tty available at " + os.ttyname(slave_fd))

    play_sequence(master_fd, itertools.repeat(SLIDER_VALUE), NUM_SLIDERS, STEP_DELAY)

if __name__ == "__main__": 
    main()