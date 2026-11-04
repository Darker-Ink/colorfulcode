-- 8-bit counter with synchronous reset
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity counter is
  generic (
    WIDTH : natural := 8
  );
  port (
    clk    : in  std_logic;
    rst    : in  std_logic;
    enable : in  std_logic;
    count  : out std_logic_vector(WIDTH - 1 downto 0)
  );
end entity counter;

architecture rtl of counter is
  type state_t is (IDLE, RUNNING, DONE);
  signal state : state_t := IDLE;
  signal value : unsigned(WIDTH - 1 downto 0) := (others => '0');
  constant LIMIT : integer := 16#F0#;
begin

  p_count : process (clk)
  begin
    if rising_edge(clk) then
      if rst = '1' then
        value <= (others => '0');
        state <= IDLE;
      elsif enable = '1' and value < LIMIT then
        value <= value + 1;
        state <= RUNNING;
      else
        state <= DONE;
      end if;
    end if;
  end process p_count;

  count <= std_logic_vector(value);
  assert value'length = WIDTH report "width mismatch" severity error;
  /* VHDL-2008 block comment */
  -- bit string literal
  -- mask <= x"FF";
end architecture rtl;
