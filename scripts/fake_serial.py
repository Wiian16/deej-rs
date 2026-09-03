import itertools
import os
import pty
import time

# Maximum value to set sliders to
SLIDER_MAX = 1023
# Number of sliders to emulate
NUM_SLIDERS = 5
# Delay in seconds between ouputting each line
STEP_DELAY = 0.01
# Number of writes to hold at each extreme
HOLD_DURATION = 200 


def gen_slider_line(value: int, sliders: int) -> bytes:
    return ("|".join([str(value)] * sliders) + "\n").encode()


def play_sequence(fd: int, values, sliders: int, delay: float):
    """Write one slider-state line per value in `values`, sleeping `delay` between writes."""
    for value in values:
        os.write(fd, gen_slider_line(value, sliders))
        time.sleep(delay)


def sweep_cycle():
    """One full up/hold/down/hold cycle of slider values."""
    up = range(0, SLIDER_MAX + 1)
    hold_max = itertools.repeat(SLIDER_MAX, HOLD_DURATION)
    down = reversed(range(0, SLIDER_MAX + 1))
    hold_min = itertools.repeat(0, HOLD_DURATION)
    return itertools.chain(up, hold_max, down, hold_min)


def main():
    master_fd, slave_fd = pty.openpty()
    print("Emulated tty available at " + os.ttyname(slave_fd))

    while True:
        play_sequence(master_fd, sweep_cycle(), NUM_SLIDERS, STEP_DELAY)


if __name__ == "__main__":
    main()