import enum

class Entry:
  def __init__(self):
    pass

  @staticmethod
  def decode(data):
    return Entry()

  def encode(self):
    data = dict()

    return data

  def __repr__(self):
    return "<Entry>".format()

class Type:
  def __init__(self):
    pass

  @staticmethod
  def decode(data):
    return Type()

  def encode(self):
    data = dict()

    return data

  def __repr__(self):
    return "<Type>".format()

  def type_method(self):
    pass

class Interface:
  @staticmethod
  def decode(data):
    f_tag = data["type"]

    if f_tag == "SubType":
      return Interface_SubType.decode(data)

    raise Exception("bad type: " + f_tag)

  def interface_method(self):
    pass

class Interface_SubType:
  TYPE = "SubType"

  def __init__(self):
    pass

  @staticmethod
  def decode(data):
    return Interface_SubType()

  def encode(self):
    data = dict()

    data["type"] = "SubType"

    return data

  def __repr__(self):
    return "<Interface_SubType>".format()

  def subtype_method(self):
    pass

class Enum:
  def __init__(self, ordinal):
    self.ordinal = ordinal

  def get_ordinal(self):
    return self.ordinal

  def enum_method(self):
    pass

  def encode(self):
    return self.ordinal

  @classmethod
  def decode(cls, data):
    for value in cls.__members__.values():
      if value.ordinal == data:
        return value

    raise Exception("data does not match enum")

  def __repr__(self):
    return "<Enum ordinal:{!r}>".format(self.ordinal)

class Tuple:
  def __init__(self):
    pass

  def tuple_method(self):
    pass

  @staticmethod
  def decode(data):
    return Tuple()

  def encode(self):
    return ()

  def __repr__(self):
    return "<Tuple>".format()

Enum = enum.Enum("Enum", [("Variant", "Variant")], type=Enum)
