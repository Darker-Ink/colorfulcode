' Inventory management sample
Imports System
Imports System.Collections.Generic

#Region "Models"
Public Class Product
    Public Property Name As String
    Public Property Price As Decimal
    Private ReadOnly _tags As New List(Of String)

    Public Sub New(name As String, price As Decimal)
        Me.Name = name
        Me.Price = price
    End Sub

    Public Function Discounted(Optional percent As Integer = 10) As Decimal
        Return Price * (100 - percent) / 100D
    End Function
End Class
#End Region

Module Program
    Sub Main()
        Dim items = New List(Of Product) From {
            New Product("Widget", 9.99D),
            New Product("Gadget", 24.5D)
        }
        Dim mask As Integer = &HFF
        Dim release As Date = #2024-01-15#

        For Each p As Product In items
            If p.Price > 10 AndAlso Not p Is Nothing Then
                Console.WriteLine($"{p.Name}: {p.Discounted():C2}")
            Else
                Console.WriteLine("Cheap: " & p.Name & " said ""hi""")
            End If
        Next

        REM old style comment
        Try
            Dim x = CInt("abc")
        Catch ex As FormatException
            Exit Sub
        End Try
    End Sub
End Module
