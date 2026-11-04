; ModuleID = 'hello.c'
source_filename = "hello.c"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Point = type { i32, double }

@.str = private unnamed_addr constant [14 x i8] c"Hello, %s!\0A\00", align 1
@counter = dso_local global i32 0, align 4

; Function Attrs: noinline nounwind
define dso_local i32 @add(i32 noundef %a, i32 noundef %b) #0 {
entry:
  %sum = add nsw i32 %a, %b
  %cmp = icmp sgt i32 %sum, 100
  br i1 %cmp, label %big, label %small

big:
  %0 = load i32, ptr @counter, align 4
  %inc = add i32 %0, 1
  store i32 %inc, ptr @counter, align 4
  ret i32 %sum

small:
  %p = alloca %struct.Point, align 8
  %y = getelementptr inbounds %struct.Point, ptr %p, i32 0, i32 1
  store double 3.5e-1, ptr %y, align 8
  %call = call i32 (ptr, ...) @printf(ptr noundef @.str, ptr null)
  %r = select i1 true, i32 -1, i32 %sum
  ret i32 %r, !dbg !12
}

declare i32 @printf(ptr noundef, ...)

attributes #0 = { noinline nounwind "frame-pointer"="all" }
!12 = !DILocation(line: 7, column: 3, scope: !4)
!llvm.ident = !{!"clang version 17.0.0"}
