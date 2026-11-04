/*
 * Calculator service definition.
 */
include "shared.thrift"

namespace java com.example.calc
namespace py calc

// Constants
const i32 MAX_VALUE = 1000
const string GREETING = "hello \"world\""
const map<string, string> MAP_CONST = {"hello": "world", "goodnight": "moon"}

typedef i64 Timestamp

enum Operation {
  ADD = 1,
  SUBTRACT = 2,
  MULTIPLY = 3,
  DIVIDE = 4
}

struct Work {
  1: i32 num1 = 0,
  2: i32 num2,
  3: Operation op,
  4: optional string comment,
  5: required list<double> values,
  6: set<binary> blobs
}

exception InvalidOperation {
  1: i32 whatOp,
  2: string why
}

# Shell style comment
service Calculator extends shared.SharedService {
  void ping(),
  i32 add(1: i32 num1, 2: i32 num2),
  i32 calculate(1: i32 logid, 2: Work w) throws (1: InvalidOperation ouch),
  oneway void zip()
}
