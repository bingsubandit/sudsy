class A < Exception; end
#         ^^^^^^^^^ inherit-exception
class B < StandardError
  def initialize(msg)
# ^ useless-method-definition
    super(msg)
  end
  def foo=(_v)
    return 1
#   ^^^^^^ return-in-void-context
  end
  def bar(a, b = 1)
    super
  end
  private
  def self.baz; end
# ^^^ ineffective-access-modifier
  private
# ^^^^^^^ useless-access-modifier
  def qux; end
  public
# ^^^^^^ useless-access-modifier
end
class C
  def run
# ^ useless-method-definition
    super
  end
  private
# ^^^^^^^ useless-access-modifier
end
class D < B
  def initialize
# ^ missing-super
  end
end
[1].each { X = 1 }
#          ^^^^^ constant-definition-in-block
