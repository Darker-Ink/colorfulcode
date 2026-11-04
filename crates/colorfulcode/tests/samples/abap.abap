*&---------------------------------------------------------------------*
*& Report ZFLIGHT_LIST
*&---------------------------------------------------------------------*
REPORT zflight_list.

TYPES: BEGIN OF ty_flight,
         carrid TYPE s_carr_id,
         connid TYPE s_conn_id,
         price  TYPE p LENGTH 8 DECIMALS 2,
       END OF ty_flight,
       tt_flights TYPE STANDARD TABLE OF ty_flight WITH DEFAULT KEY.

DATA: lt_flights TYPE tt_flights,
      lv_total   TYPE p DECIMALS 2,
      lv_text    TYPE string.

PARAMETERS p_carr TYPE s_carr_id DEFAULT 'LH'.

CLASS lcl_report DEFINITION.
  PUBLIC SECTION.
    METHODS run IMPORTING iv_carrier TYPE s_carr_id
                RETURNING VALUE(rv_count) TYPE i.
ENDCLASS.

CLASS lcl_report IMPLEMENTATION.
  METHOD run.
    SELECT carrid, connid, price
      FROM sflight
      WHERE carrid = @iv_carrier
      INTO TABLE @lt_flights.

    LOOP AT lt_flights ASSIGNING FIELD-SYMBOL(<ls_flight>).
      lv_total = lv_total + <ls_flight>-price.
    ENDLOOP.
    rv_count = lines( lt_flights ).
  ENDMETHOD.
ENDCLASS.

START-OF-SELECTION.
  DATA(lo_report) = NEW lcl_report( ).
  DATA(lv_count) = lo_report->run( p_carr ).
  lv_text = |Found { lv_count } flights, total { lv_total }|.
  IF lv_count > 0 AND lv_total IS NOT INITIAL.
    WRITE: / lv_text, 'It''s done'. " trailing comment
  ENDIF.
  cl_demo_output=>display( lt_flights ) ##NO_TEXT.
