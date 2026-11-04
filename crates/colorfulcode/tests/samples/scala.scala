package shapes

import scala.annotation.tailrec
import scala.collection.mutable

/** Geometric shapes. */
sealed trait Shape:
  def area: Double

final case class Circle(radius: Double) extends Shape:
  def area: Double = math.Pi * radius * radius

final case class Rect(w: Double, h: Double) extends Shape:
  def area: Double = w * h

object Shapes {
  val MaxShapes = 100
  private val cache = mutable.Map.empty[String, Shape]

  def describe(s: Shape): String = s match {
    case Circle(r) if r > 10.0 => s"big circle of radius $r"
    case Circle(r)             => f"circle ${r}%.2f"
    case Rect(w, h)            => s"rect ${w}x${h}"
  }

  @tailrec
  def sum(xs: List[Int], acc: Int = 0): Int = xs match {
    case Nil          => acc
    case head :: tail => sum(tail, acc + head)
  }

  def main(args: Array[String]): Unit = {
    val shapes: Seq[Shape] = Seq(Circle(1.5), Rect(2, 3))
    shapes.map(describe).foreach(println)
    val raw = """no \n escapes here"""
    val path = raw"C:\temp\$raw"
    val c = 'x'
    val nl = '\n'
    println(sum(List(1, 2, 3)) + 0xFF + raw.length)
  }
}
