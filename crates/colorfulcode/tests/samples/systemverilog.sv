// FIFO with an interface and a class based test
`include "defs.svh"

interface fifo_if #(parameter int WIDTH = 8) (input logic clk);
  logic             push, pop, full, empty;
  logic [WIDTH-1:0] wdata, rdata;
  modport dut (input push, pop, wdata, output full, empty, rdata);
endinterface

typedef enum logic [1:0] {IDLE, BUSY, ERROR} state_t;

module fifo #(parameter int DEPTH = 16) (fifo_if.dut bus, input logic rst_n);
  logic [7:0] mem [DEPTH];
  int unsigned count;

  always_ff @(posedge bus.clk or negedge rst_n) begin
    if (!rst_n) count <= '0;
    else if (bus.push && !bus.full) begin
      mem[count] <= bus.wdata;
      count <= count + 1;
    end
  end

  always_comb bus.full = (count == DEPTH);
  assert property (@(posedge bus.clk) !(bus.push && bus.full))
    else $error("overflow at %t", $time);
endmodule

class packet extends base_item;
  rand bit [7:0] payload[];
  string name = "pkt";
  constraint c_len { payload.size() inside {[1:16]}; }

  function new(string name = "packet");
    super.new();
    this.name = name;
  endfunction

  virtual task send(ref logic [7:0] q[$]);
    foreach (payload[i]) q.push_back(payload[i]);
  endtask
endclass
