! Numerical integration with the trapezoidal rule
module integrate_mod
  use iso_fortran_env, only: dp => real64
  implicit none
  private
  public :: trapezoid, point

  type :: point
    real(dp) :: x = 0.0_dp, y = 0.0_dp
  end type point

  integer, parameter :: MAX_STEPS = 1000

contains

  pure function f(x) result(y)
    real(dp), intent(in) :: x
    real(dp) :: y
    y = exp(-x**2) * cos(3.0d0 * x)
  end function f

  subroutine trapezoid(a, b, n, total)
    real(dp), intent(in) :: a, b
    integer, intent(in) :: n
    real(dp), intent(out) :: total
    integer :: i
    real(dp) :: h
    type(point) :: p

    h = (b - a) / real(n, dp)
    total = 0.5_dp * (f(a) + f(b))
    do i = 1, n - 1
      if (i > MAX_STEPS .and. .not. (n == 0)) exit
      total = total + f(a + i * h)
    end do
    total = total * h
    p%x = total
    print '(A, F12.6)', 'Integral = ', total
    write (*, *) "Done: ""quoted"" text", .true.
  end subroutine trapezoid

end module integrate_mod

program main
  use integrate_mod
  real(8) :: r
  call trapezoid(0.0d0, 1.0d0, 100, r)
end program main
